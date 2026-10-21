//! What the mothership sells and what the pod carries (`docs/spec/economy.md` 6): the stock of
//! each shop, the prices, buying, and fitting equipment to the pod (EQUIP POD). The screens
//! that show it belong to the scene layer; this is the state behind them.

use super::Seed;
use super::objects::Score;

/// Items of the shop numbered as in economy.md 6.2: 0-11 ordinary, 12 and 13 the mapping
/// devices, 14 and 15 the jetpods.
pub const ITEMS: usize = 16;
pub const MAPPING_DEVICE: usize = 12;
pub const SUPER_MAPPING_DEVICE: usize = 13;
pub const NIPPON_JETPOD: usize = 14;
pub const ITALIAN_JETPOD: usize = 15;

/// The most of one ordinary item the pod can own.
const MOST_OWNED: u8 = 9;

/// Prices (economy.md 6.2).
const PRICES: [u32; ITEMS] = [
    400, 1000, 2500, 150, 400, 400, 800, 1600, 500, 800, 500, 1000, 1500, 5000, 10000, 25000,
];

/// The special weapon (RAM $D1) each of the items 0-5 fits (economy.md 6.4).
pub(crate) const WEAPON_CODES: [u8; 6] = [1, 2, 4, 5, 8, 6];

/// The stock lists of the four systems (economy.md 6.1), as item numbers; the shops of system
/// `s` offer the lists of systems 0 to `s` in item order.
const STOCK: [&[usize]; 4] = [&[0, 3, 6, 10, 12], &[1, 5, 8, 13], &[4, 9, 11], &[2, 7]];

/// The name of each item (text.md 4.5).
pub const NAMES: [&str; ITEMS] = [
    "HOMING MISSILES",
    "ANTI GRAVITY",
    "SMART BOMBS",
    "TIME BOMBS",
    "STAR BULLETS",
    "MULTI WARHEAD MISSILES",
    "TITANIUM BULLET PACK",
    "MILITARY BULLET SYSTEM",
    "SUPER SHIELDS",
    "MOMENTUM KILLER",
    "EFFICIENT ENGINES",
    "DOUBLE STRENGTH THRUSTERS",
    "MAPPING DEVICE",
    "SUPER MAPPING DEVICE",
    "NIPPON SPORTS JETPOD",
    "ITALIAN RACING JETPOD",
];

pub fn price(item: usize) -> u32 {
    PRICES[item]
}

/// What a piece of equipment is fitted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Slot {
    /// RAM $D1.
    Weapon(u8),
    /// RAM $050E.
    Bullets(u8),
    /// RAM $D3.
    Device(u8),
}

fn slot_of(item: usize) -> Option<Slot> {
    match item {
        0..=5 => Some(Slot::Weapon(WEAPON_CODES[item])),
        6 | 7 => Some(Slot::Bullets(item as u8 - 5)),
        8..=11 => Some(Slot::Device(item as u8)),
        _ => None,
    }
}

/// The owned counts RAM $0600-$060D, the pod's fixed fittings and what is fitted now.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Inventory {
    /// Owned counts of items 0-13 (the mapping devices 0 or 1).
    pub counts: [u8; 14],
    /// RAM $DC: pod type (bits 0-1), boosters (bit 6), probe shields (bit 7).
    pub pod: u8,
    /// RAM $D1, $050E and $D3.
    pub weapon: u8,
    pub bullets: u8,
    pub device: u8,
}

/// Why a purchase was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Refusal {
    TooPoor,
    TooMany,
    AlreadyOwned,
}

impl Inventory {
    /// The pod type, 0-2.
    pub fn pod_type(&self) -> u8 {
        self.pod & 3
    }

    /// The items a shop on `planet` offers, in order, as it builds its list when it opens
    /// (economy.md 6.1): hidden entries are left out and, on planets 3, 7 and 11, the mapping
    /// devices' places go to the jetpods.
    pub fn stock(&self, planet: usize) -> Vec<usize> {
        let system = (planet / 3).min(3);
        let jetpods = planet & 3 == 3;
        let mut items: Vec<usize> = STOCK[..=system]
            .iter()
            .flat_map(|l| l.iter().copied())
            .collect();
        items.sort_unstable();
        items.retain_mut(|item| {
            if jetpods && *item >= MAPPING_DEVICE {
                *item += 2;
            }
            match *item {
                MAPPING_DEVICE => {
                    self.counts[MAPPING_DEVICE] + self.counts[SUPER_MAPPING_DEVICE] == 0
                }
                SUPER_MAPPING_DEVICE => self.counts[SUPER_MAPPING_DEVICE] == 0,
                NIPPON_JETPOD => self.pod_type() == 0,
                ITALIAN_JETPOD => self.pod_type() != 2,
                _ => true,
            }
        });
        items
    }

    /// How many of `item` the shop says are owned ("YOU OWN n OF THEM"; a jetpod: 1 when its
    /// bit is held, the Italian showing 0 for the Nippon).
    pub fn owned(&self, item: usize) -> u8 {
        match item {
            0..=13 => self.counts[item],
            NIPPON_JETPOD => u8::from(self.pod & 1 != 0),
            _ => u8::from(self.pod & 2 != 0),
        }
    }

    /// Whether `item` can be bought with `score` (economy.md 6.3).
    pub fn check(&self, item: usize, score: Score) -> Result<(), Refusal> {
        if score.0 < price(item) {
            return Err(Refusal::TooPoor);
        }
        match item {
            0..=11 if self.counts[item] >= MOST_OWNED => Err(Refusal::TooMany),
            MAPPING_DEVICE | SUPER_MAPPING_DEVICE if self.counts[item] != 0 => {
                Err(Refusal::AlreadyOwned)
            }
            NIPPON_JETPOD if self.pod_type() != 0 => Err(Refusal::AlreadyOwned),
            ITALIAN_JETPOD if self.pod_type() == 2 => Err(Refusal::AlreadyOwned),
            _ => Ok(()),
        }
    }

    /// Buys `item`: the price comes off the score and the item is owned (a jetpod replaces the
    /// pod type, keeping the shields and boosters).
    pub fn buy(&mut self, item: usize, score: &mut Score) -> Result<(), Refusal> {
        self.check(item, *score)?;
        score.0 -= price(item);
        self.grant(item);
        Ok(())
    }

    /// Makes `item` owned: the last step of a purchase, once the shop has taken the price
    /// digit by digit (economy.md 6.3).
    pub fn grant(&mut self, item: usize) {
        match item {
            0..=13 => self.counts[item] += 1,
            NIPPON_JETPOD => self.pod = (self.pod & 0xC0) + 1,
            _ => self.pod = (self.pod & 0xC0) + 2,
        }
    }

    /// The item number of what is fitted in `slot`, if anything.
    fn fitted_item(&self, slot: Slot) -> Option<usize> {
        match slot {
            Slot::Weapon(_) => WEAPON_CODES
                .iter()
                .position(|&c| c == self.weapon && c != 0),
            Slot::Bullets(_) => match self.bullets {
                1 => Some(6),
                2 => Some(7),
                _ => None,
            },
            Slot::Device(_) => (8..=11)
                .contains(&self.device)
                .then(|| usize::from(self.device)),
        }
    }

    fn slot_value(&mut self, slot: Slot) -> &mut u8 {
        match slot {
            Slot::Weapon(_) => &mut self.weapon,
            Slot::Bullets(_) => &mut self.bullets,
            Slot::Device(_) => &mut self.device,
        }
    }

    /// Returns everything fitted to the owned counts (what opening the shop or the equip list
    /// does first).
    pub fn unfit_all(&mut self) {
        for slot in [Slot::Weapon(0), Slot::Bullets(0), Slot::Device(0)] {
            if let Some(item) = self.fitted_item(slot) {
                self.counts[item] += 1;
            }
            *self.slot_value(slot) = 0;
        }
    }

    /// The items the equip list shows: the owned items 0-11, built once when it opens.
    pub fn equip_list(&self) -> Vec<usize> {
        (0..12).filter(|&i| self.counts[i] != 0).collect()
    }

    /// A on `item` in the equip list (economy.md 6.4): with none left it does nothing; what the
    /// slot held goes back to its count first, and choosing what is already fitted removes it.
    pub fn fit(&mut self, item: usize) {
        let Some(slot) = slot_of(item) else { return };
        if self.counts[item] == 0 {
            return;
        }
        let held = self.fitted_item(slot);
        if let Some(held) = held {
            self.counts[held] += 1;
        }
        if held == Some(item) {
            *self.slot_value(slot) = 0;
            return;
        }
        self.counts[item] -= 1;
        *self.slot_value(slot) = match slot {
            Slot::Weapon(code) | Slot::Bullets(code) | Slot::Device(code) => code,
        };
    }

    /// Sets what the next level starts with.
    pub fn into_seed(&self, seed: &mut Seed) {
        seed.equipment = self.pod;
        seed.weapon = self.weapon;
        seed.bullet_upgrade = self.bullets;
        seed.device = self.device;
        seed.mapping_device = self.counts[MAPPING_DEVICE] != 0;
        seed.super_mapping_device = self.counts[SUPER_MAPPING_DEVICE] != 0;
    }

    /// Takes back what play ended with: the pod's fittings (lost with the pod, and the bullet
    /// upgrade with Jetman), the shields and boosters found, the mapping devices delivered.
    pub fn take_from_seed(&mut self, seed: &Seed) {
        self.pod = seed.equipment;
        self.weapon = seed.weapon;
        self.bullets = seed.bullet_upgrade;
        self.device = seed.device;
        self.counts[MAPPING_DEVICE] = u8::from(seed.mapping_device);
        self.counts[SUPER_MAPPING_DEVICE] = u8::from(seed.super_mapping_device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_is_cumulative_and_hides_what_is_owned() {
        let mut inventory = Inventory::default();
        assert_eq!(inventory.stock(1), [0, 3, 6, 10, 12]);
        assert_eq!(inventory.stock(3), [0, 1, 3, 5, 6, 8, 10, 14, 15]);
        assert_eq!(inventory.stock(7), [0, 1, 3, 4, 5, 6, 8, 9, 10, 11, 14, 15]);
        assert_eq!(inventory.stock(9).len(), 14);
        inventory.counts[MAPPING_DEVICE] = 1;
        assert_eq!(inventory.stock(5), [0, 1, 3, 5, 6, 8, 10, 13]);
        inventory.pod = 1;
        assert_eq!(inventory.stock(11).last(), Some(&15));
        inventory.pod = 2;
        assert!(!inventory.stock(11).contains(&15));
    }

    #[test]
    fn buying_follows_the_score_and_the_counts() {
        let mut inventory = Inventory::default();
        let mut score = Score(499);
        assert_eq!(inventory.buy(8, &mut score), Err(Refusal::TooPoor));
        score = Score(700);
        assert_eq!(inventory.buy(8, &mut score), Ok(()));
        assert_eq!((score.0, inventory.owned(8)), (200, 1));
        inventory.counts[3] = 9;
        score = Score(5000);
        assert_eq!(inventory.buy(3, &mut score), Err(Refusal::TooMany));
        assert_eq!(inventory.buy(12, &mut score), Ok(()));
        assert_eq!(inventory.buy(12, &mut score), Err(Refusal::AlreadyOwned));
    }

    #[test]
    fn a_jetpod_keeps_the_shields_and_boosters() {
        let mut inventory = Inventory {
            pod: 0xC0,
            ..Inventory::default()
        };
        let mut score = Score(90000);
        inventory.buy(NIPPON_JETPOD, &mut score).unwrap();
        assert_eq!((inventory.pod, inventory.owned(NIPPON_JETPOD)), (0xC1, 1));
        inventory.buy(ITALIAN_JETPOD, &mut score).unwrap();
        assert_eq!((inventory.pod, inventory.owned(NIPPON_JETPOD)), (0xC2, 0));
        assert_eq!(
            inventory.buy(ITALIAN_JETPOD, &mut score),
            Err(Refusal::AlreadyOwned)
        );
    }

    #[test]
    fn fitting_swaps_toggles_and_unfits() {
        let mut inventory = Inventory::default();
        inventory.counts[1] = 2;
        inventory.counts[5] = 1;
        inventory.counts[10] = 1;
        inventory.counts[7] = 1;
        inventory.fit(1);
        inventory.fit(10);
        inventory.fit(7);
        assert_eq!(
            (inventory.weapon, inventory.device, inventory.bullets),
            (2, 10, 2)
        );
        inventory.fit(5);
        assert_eq!(
            (inventory.weapon, inventory.counts[1], inventory.counts[5]),
            (6, 2, 0)
        );
        inventory.fit(1);
        assert_eq!((inventory.weapon, inventory.counts[5]), (2, 1));
        inventory.fit(1);
        assert_eq!((inventory.weapon, inventory.counts[1]), (0, 2));
        inventory.unfit_all();
        assert_eq!(
            (inventory.device, inventory.bullets, inventory.counts[10]),
            (0, 0, 1)
        );
        assert_eq!(inventory.counts[7], 1);
    }

    #[test]
    fn an_item_with_none_left_cannot_be_unfitted() {
        let mut inventory = Inventory::default();
        inventory.counts[9] = 1;
        inventory.fit(9);
        assert_eq!((inventory.device, inventory.counts[9]), (9, 0));
        inventory.fit(9);
        assert_eq!(inventory.device, 9);
    }

    #[test]
    fn the_seed_carries_the_fittings() {
        let mut inventory = Inventory {
            pod: 0xC2,
            ..Inventory::default()
        };
        inventory.counts[2] = 1;
        inventory.counts[SUPER_MAPPING_DEVICE] = 1;
        inventory.fit(2);
        let mut seed = Seed::default();
        inventory.into_seed(&mut seed);
        assert_eq!((seed.equipment, seed.weapon), (0xC2, 4));
        assert!(seed.super_mapping_device && !seed.mapping_device);
        seed.weapon = 0;
        inventory.take_from_seed(&seed);
        assert_eq!(inventory.weapon, 0);
    }
}
