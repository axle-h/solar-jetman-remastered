//! Helpers for saving the game's state with serde: `with` modules for arrays longer than the 32
//! elements serde supports on its own, the header of the save-state format, and what lets a
//! saved state leave out everything imported from the ROM (tiles, colours, pictures): the
//! types that hold such data save a short recipe and, while a state is loaded, find the
//! imported assets here to rebuild it.

/// Version of the save-state format; bump it whenever the serialised state changes shape.
pub const FORMAT_VERSION: u32 = 5;
/// Version of the Scaled engine's state (`crate::game::scaled`), which follows the format version in
/// the header; bump it whenever the Scaled state changes shape. Its state is rate-free: game
/// time and values in their Q formats, never a tick count or the rate.
pub const SCALED_FORMAT_VERSION: u32 = 2;
/// The first bytes of a save state.
pub const MAGIC: &[u8; 4] = b"SJSV";

/// Why a save state could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateError {
    /// Not a save state of this game.
    NotASaveState,
    /// Written by a different version of the format.
    Version(u32),
    /// Written by a different version of the Scaled engine's state.
    ScaledVersion(u32),
    /// The data is damaged.
    Damaged(String),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::NotASaveState => f.write_str("not a save state"),
            StateError::Version(v) => write!(f, "save state format {v} is not supported"),
            StateError::ScaledVersion(v) => {
                write!(
                    f,
                    "save state of the Scaled engine version {v} is not supported"
                )
            }
            StateError::Damaged(why) => write!(f, "damaged save state: {why}"),
        }
    }
}

impl std::error::Error for StateError {}

fn to_array<T, const N: usize, E: serde::de::Error>(v: Vec<T>) -> Result<[T; N], E> {
    let len = v.len();
    v.try_into()
        .map_err(|_| E::invalid_length(len, &"an array of the right length"))
}

/// `#[serde(with = "crate::game::state::array")]` for `[T; N]`.
pub mod array {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize, const N: usize>(
        a: &[T; N],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        a.as_slice().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
        d: D,
    ) -> Result<[T; N], D::Error> {
        super::to_array(Vec::<T>::deserialize(d)?)
    }
}

/// For `Vec<[T; N]>` with any N.
pub mod vec_array {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize, const N: usize>(
        v: &[[T; N]],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|a| a.as_slice()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T: Deserialize<'de>, const N: usize>(
        d: D,
    ) -> Result<Vec<[T; N]>, D::Error> {
        Vec::<Vec<T>>::deserialize(d)?
            .into_iter()
            .map(super::to_array)
            .collect()
    }
}

/// For `[[T; M]; N]` with any M and N.
pub mod nested {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer, T: Serialize, const M: usize, const N: usize>(
        v: &[[T; M]; N],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_seq(v.iter().map(|a| a.as_slice()))
    }

    pub fn deserialize<
        'de,
        D: Deserializer<'de>,
        T: Deserialize<'de>,
        const M: usize,
        const N: usize,
    >(
        d: D,
    ) -> Result<[[T; M]; N], D::Error> {
        let rows: Vec<[T; M]> = Vec::<Vec<T>>::deserialize(d)?
            .into_iter()
            .map(super::to_array)
            .collect::<Result<_, _>>()?;
        super::to_array(rows)
    }
}

/// The same text always gives the same `&'static str`, so loading states over and over does not
/// leak more than the distinct strings the game has.
pub(crate) fn intern(text: &str) -> &'static str {
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};
    static TABLE: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut table = TABLE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(known) = table.get(text) {
        return known;
    }
    let leaked: &'static str = Box::leak(text.to_owned().into_boxed_str());
    table.insert(leaked);
    leaked
}

/// For a `Vec<Option<&'static str>>`.
pub mod static_str_rows {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        rows: &[Option<&'static str>],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        rows.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Vec<Option<&'static str>>, D::Error> {
        Ok(Vec::<Option<String>>::deserialize(d)?
            .into_iter()
            .map(|row| row.map(|text| super::intern(&text)))
            .collect())
    }
}

thread_local! {
    static ASSETS: std::cell::RefCell<Option<std::rc::Rc<crate::game::scene::Assets>>> =
        const { std::cell::RefCell::new(None) };
}

/// Runs `load` with `assets` available to [`assets`].
pub(crate) fn with_assets<T>(assets: &crate::game::scene::Assets, load: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ASSETS.with(|slot| slot.borrow_mut().take());
        }
    }
    ASSETS.with(|slot| *slot.borrow_mut() = Some(std::rc::Rc::new(assets.clone())));
    let _reset = Reset;
    load()
}

/// The imported assets a state is being loaded with.
pub(crate) fn assets<E: serde::de::Error>() -> Result<std::rc::Rc<crate::game::scene::Assets>, E> {
    ASSETS
        .with(|slot| slot.borrow().clone())
        .ok_or_else(|| E::custom("a saved state can only be loaded with the imported assets"))
}

/// Defines a `with` module for a field that is a copy of imported data: nothing is saved, and
/// loading takes the value from the assets' graphics.
macro_rules! from_graphics {
    ($name:ident: $ty:ty = |$graphics:ident| $value:expr) => {
        pub mod $name {
            use serde::{Deserialize, Deserializer, Serializer};

            pub fn serialize<S: Serializer>(_: &$ty, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_unit()
            }

            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<$ty, D::Error> {
                <()>::deserialize(d)?;
                let assets = super::assets::<D::Error>()?;
                let $graphics = &assets.graphics;
                Ok($value)
            }
        }
    };
}

from_graphics!(attract_nametable: Vec<u8> = |g| g.attract_nametable.clone());
from_graphics!(attract_attributes: Vec<u8> = |g| g.attract_attributes.clone());
from_graphics!(backdrop: crate::game::graphics::Backdrop = |g| g.backdrop.clone());
from_graphics!(title_fade: Vec<Vec<u8>> = |g| g.title_fade.clone());
from_graphics!(title_fade_end: Vec<u8> = |g| g.title_fade_end.clone());
from_graphics!(flight_window: Vec<u8> = |g| g.flight_window.clone());
from_graphics!(flight_sine: Vec<u8> = |g| g.flight_sine.clone());
from_graphics!(gauge_rows: Vec<u8> = |g| g.gauge_rows.clone());
