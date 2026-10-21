//! The controller in the Scaled engine (`docs/scaled.md`): read every tick, presses kept
//! until a tick and a step have seen them, and the held buttons that act once per step in the
//! Original (rotation, autofire) turned into game-time cadences that act on the first tick.

use super::input::Buttons;
use super::motion::ONE;

/// Half a step: an accumulator seeded `f - HALF` on a new press acts again after half a step
/// less one tick, which centres the count of a hold on the Original's (`docs/scaled.md`).
const HALF: i32 = 1 << 15;

/// Where the controller is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Controls {
    /// Held at the last tick.
    pub held: Buttons,
    /// Newly pressed at the last tick, including presses that came and went before it.
    pub pressed: Buttons,
    /// Presses since the step logic last took them (`take_step_presses`).
    step_pressed: Buttons,
    /// Presses since the last frame event (the random generator reads them).
    frame_pressed: Buttons,
    /// The rotation's direction at the last tick (-1 left, 1 right, 0 none) and its cadence
    /// (Q16 of a step).
    turning: i8,
    turn: i32,
    /// The gun's cadence (Q16 of a step) while A is held.
    firing: bool,
    fire: i32,
}

impl Controls {
    /// Reads the controller for a tick. `buttons` holds what is held now and anything pressed
    /// since the last read, so a tap shorter than a tick still counts as a press.
    pub fn read(&mut self, buttons: Buttons, now_held: Buttons) {
        self.pressed = Buttons(buttons.0 & !self.held.0);
        self.held = now_held;
        self.step_pressed.0 |= self.pressed.0;
        self.frame_pressed.0 |= self.pressed.0;
    }

    /// The presses since the step logic last took them.
    pub fn take_step_presses(&mut self) -> Buttons {
        std::mem::take(&mut self.step_pressed)
    }

    /// The presses since the last frame event.
    pub fn take_frame_presses(&mut self) -> Buttons {
        std::mem::take(&mut self.frame_pressed)
    }

    /// Forgets the presses and the cadences (a new scene's input: nothing counts as pressed).
    pub fn settle(&mut self, held: Buttons) {
        *self = Controls {
            held,
            ..Controls::default()
        };
    }

    /// The facing change of this tick of fraction `f`: a new press of Left or Right turns at
    /// once, a hold once per step of game time (Left wins, as in the Original). A press that
    /// came and went between ticks turns once.
    pub fn rotation(&mut self, f: u32) -> i8 {
        let direction = if self.held.has(Buttons::LEFT) {
            -1
        } else if self.held.has(Buttons::RIGHT) {
            1
        } else {
            0
        };
        let before = std::mem::replace(&mut self.turning, direction);
        if direction == 0 {
            // A tap that ended before this tick.
            return if self.pressed.has(Buttons::LEFT) {
                -1
            } else {
                i8::from(self.pressed.has(Buttons::RIGHT))
            };
        }
        if before == 0 {
            self.turn = f as i32 - HALF;
            return direction;
        }
        // Left to Right with no tick in between is not a new press: the cadence goes on.
        self.turn += f as i32;
        if self.turn >= ONE as i32 {
            self.turn -= ONE as i32;
            direction
        } else {
            0
        }
    }

    /// Whether the gun fires in this tick: a new press of A at once, and while A is held (not
    /// with Select) every `period` steps of game time (6, 4 or 2 by the pod type).
    pub fn fire(&mut self, f: u32, period: u32) -> bool {
        let held = self.held.has(Buttons::A) && !self.held.has(Buttons::SELECT);
        let press = self.pressed.has(Buttons::A);
        let was = std::mem::replace(&mut self.firing, held);
        if press || (held && !was) {
            self.fire = f as i32 - HALF;
            return true;
        }
        if !held {
            return false;
        }
        self.fire += f as i32;
        let every = (period * ONE) as i32;
        if self.fire >= every {
            self.fire -= every;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Holds `buttons` for `ticks` ticks at `rate` and counts the facing change and the shots.
    fn hold(controls: &mut Controls, buttons: Buttons, rate: u32, ticks: u32) -> (i32, u32) {
        let f = ONE * 30 / rate;
        let (mut turned, mut shots) = (0, 0);
        for _ in 0..ticks {
            controls.read(buttons, buttons);
            turned += i32::from(controls.rotation(f));
            shots += u32::from(controls.fire(f, 4));
        }
        (turned, shots)
    }

    #[test]
    fn at_thirty_hertz_a_hold_turns_and_fires_as_the_original_steps() {
        let mut controls = Controls::default();
        let (turned, shots) = hold(&mut controls, Buttons(Buttons::RIGHT | Buttons::A), 30, 12);
        // The Original: a turn every step, a shot at the press and every 4 steps.
        assert_eq!((turned, shots), (12, 3));
    }

    #[test]
    fn a_hold_turns_once_per_step_of_game_time_at_any_rate() {
        for rate in [60, 75, 90, 100, 120, 144, 165, 200] {
            let mut controls = Controls::default();
            // Half a second: 15 steps.
            let (turned, _) = hold(&mut controls, Buttons(Buttons::LEFT), rate, rate / 2);
            assert!((-16..=-14).contains(&turned), "{rate} Hz: {turned}");
        }
    }

    #[test]
    fn a_tap_between_ticks_still_turns_and_fires_once() {
        let mut controls = Controls::default();
        // Pressed and released before the tick read the controller: seen as pressed only.
        controls.read(Buttons(Buttons::RIGHT | Buttons::A), Buttons(0));
        assert_eq!(controls.rotation(ONE / 4), 1);
        assert!(controls.fire(ONE / 4, 4));
        controls.read(Buttons(0), Buttons(0));
        assert_eq!(controls.rotation(ONE / 4), 0);
        assert!(!controls.fire(ONE / 4, 4));
        // The step logic and the random generator still see the press.
        assert!(controls.take_step_presses().has(Buttons::RIGHT));
        assert!(controls.take_frame_presses().has(Buttons::A));
    }
}
