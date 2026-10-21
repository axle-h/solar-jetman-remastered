//! Deliveries, docking, lives and the scene changes gameplay asks for (`docs/spec/economy.md`,
//! `docs/spec/game-flow.md` 7.4, `docs/spec/player.md` 6.3-6.4, 7).
//!
//! Scaled only *requests* a scene change ([`Transition`]); the scenes after it (the message
//! screen, the surface menu, the next level) belong to the scene layer, which calls
//! [`Scaled::resume`] to go on. The remake's stub scenes return to play at once.

use super::Scaled;
use super::camera::Camera;
use super::input::Buttons;
use super::objects::{ITEM_SLOTS, Object, SHOT_SLOTS, relative, state};
use super::pod::{Event, STATE_POD, STATE_RESPAWN};
use super::tow::{ROPE, SpinReward, beam_frame, rope_offset};
use crate::game::level::{Level, PLANETS, World};
use crate::game::sound::{Channel, Request};

pub use crate::game::gameplay::economy::CARGO_VALUES;
pub use crate::game::gameplay::economy::{Delivery, Outcome, Transition};
use crate::game::gameplay::economy::{is_delivery, kind_value, towable};

/// Whether the pod is inside the mothership's bay area (economy.md 2.2).
pub(super) fn in_bay_area(pod: &super::pod::Pod, level: &Level) -> bool {
    let dx = i32::from(relative(pod.body.x, level.home.x));
    let dy = i32::from(relative(pod.body.y, level.home.y));
    (-13..=12).contains(&dx) && (-16..=39).contains(&dy)
}

impl Scaled {
    /// The scene change waiting to be acted on, if gameplay asked for one.
    pub fn transition(&self) -> Option<Transition> {
        self.transition
    }

    /// Asks for a scene change (game-flow.md 7.4): a request for the code that was last acted on
    /// is dropped while the guard counts down.
    pub(super) fn request(&mut self, transition: Transition) {
        let code = transition.code();
        if code == self.last_transition && self.guard != 0 {
            return;
        }
        self.last_transition = code;
        if code != 1 {
            self.guard = 0xFF;
        }
        if transition == Transition::Restart {
            // The special ring sounds $36 on the noise channel (Mesen).
            self.sfx.push(Request::a(Channel::Noise, 0x36));
        }
        self.transition = Some(transition);
    }

    /// Asks for a scene change as if the pod had caused it (tests, and comparison with the
    /// original: the star marker's touch asks for [`Transition::Interlude`]).
    pub fn request_scene_change(&mut self, transition: Transition) {
        self.request(transition);
    }

    /// Whether the level's items list still holds something collectable besides entry 0: kinds
    /// 3-9, 12, 15 and 16 (game-flow.md 7.3, the automatic blast-off).
    pub fn collectables_left(&self) -> bool {
        self.objects
            .items
            .iter()
            .skip(1)
            .any(|entry| matches!(entry.kind, 3..=9 | 12 | 15 | 16))
    }

    /// The state to hand to the next level (a stub scene layer starts a fresh level with it).
    pub fn seed(&self) -> super::Seed {
        super::Seed {
            rng: self.rng,
            counter: self.counter,
            score: self.score,
            memory: super::convert(&self.memory_on_leaving()),
            star_mode: self.star_mode(),
            stars: Some(super::convert(&self.stars)),
            lives: self.lives,
            jetpods: self.jetpods,
            crystals: self.crystals,
            equipment: self.pod.equipment.owned,
            weapon: self.pod.equipment.weapon,
            device: self.pod.equipment.device,
            bullet_upgrade: self.pod.equipment.bullet_upgrade,
            weapon_gauge: Some(
                u16::from(self.pod.weapon_energy) << 8 | u16::from(self.pod.weapon_low),
            ),
            mapping_device: self.mapping_device,
            super_mapping_device: self.super_mapping_device,
            planet_flags: self.planet_flags,
            leftovers: self
                .objects
                .slots
                .iter()
                .map(|o| (o.flags_y, o.frame))
                .collect(),
            pod_leftovers: super::PodLeftovers::default(),
            stale: Vec::new(),
            arrival: None,
            saved_fuel_count: self.saved_fuel_count,
            final_entry: false,
        }
    }

    // ---- The player's events ------------------------------------------------------------------

    /// What the pod's step asked for, and the rope's part of the step (player.md 8.2).
    pub(super) fn after_player(&mut self) {
        if let Some(job) = self.pod.job_request.take() {
            self.job = job;
        }
        let events = std::mem::take(&mut self.pod.events);
        let destroyed = events.contains(&Event::PodDestroyed);
        for event in events {
            match event {
                Event::PodDestroyed => {
                    self.pod_sound();
                    self.jetpods = self.jetpods.wrapping_sub(1);
                    // 7.1: only the held slot ($66) is cleared; the rope's other bytes stay. The
                    // beam is freed, which clears its state and leaves its other bytes stale.
                    self.objects.slots[2].state = 0;
                    self.explode_player();
                }
                Event::JetmanKilled => {
                    self.pod_sound();
                    if self.lives == 1 && !self.cheats.infinite_lives {
                        self.lives = 0;
                    }
                    self.explode_player();
                }
                Event::JetmanLost => self.request(Transition::JetmanLost),
                Event::SparePodFound => self.pod_sound(),
                Event::PodAssembled => self.jetpods = self.jetpods.wrapping_add(1),
            }
        }
        // A released item moves `d` pixels on each axis toward the pod's side and starts at rest
        // (player.md 8.1).
        if let Some((slot, mut d, angle)) = self.tow.release.take() {
            let item = &mut self.objects.slots[slot];
            // 7.1 (1): an item with frame 0 is released without the push of a destroyed pod.
            if destroyed && item.frame == 0 {
                d = 0;
            }
            item.x = item.x.wrapping_add_signed(if angle < 64 { -d } else { d });
            item.y = item
                .y
                .wrapping_add_signed(if (angle + 32) & 127 < 64 { d } else { -d });
            item.speed_x = 0;
            item.speed_y = 0;
            item.work6 = 0;
        }
        if let Some((b_held, c)) = self.pod.flame_call
            && self.pod.state != STATE_POD
        {
            self.jetman_flame(b_held, c);
        }
        // The flame is slot 1, state 7 (a picture only), at the pod plus the nose offset
        // (player.md 4.8); flags X 1 mirrors it, flags Y bit 7 flips it. Its flag and frame
        // bytes are read by the lift of a towed load (player.md 8.3 b): the flame routine
        // writes them only when it runs (B held, or while the flame lingers); when the flame is
        // freed the frame is its base alone, and both stay stale after.
        if self.pod.state == STATE_POD
            && matches!(self.objects.slots[1].state, 0 | state::MOTHERSHIP)
        {
            let (x, y) = (self.pod.body.x, self.pod.body.y);
            let down = u8::from(self.pod.body.v_down);
            let flame = &mut self.objects.slots[1];
            if let Some((frame, (dx, dy), mirrored, flipped)) = self.pod.flame_picture() {
                flame.state = state::MOTHERSHIP;
                flame.x = x.wrapping_add_signed(dx as i16);
                flame.y = y.wrapping_add_signed(dy as i16);
                flame.flags_x = u8::from(mirrored);
                flame.flags_y = if flipped { 0x80 } else { 0 } | down;
                flame.frame = frame;
            } else if self.pod.flame_freed {
                flame.state = 0;
                flame.flags_y = flame.flags_y & !1 | u8::from(self.pod.body.v_down);
                flame.frame = self.pod.flame_base();
            }
        }
        if self.tow.holding() {
            self.objects.slots[2].frame = beam_frame(self.tow.angle8());
            let slot = self.tow.slot;
            let item = self.objects.slots[slot];
            if item.state == 0 {
                self.tow.slot = 0;
            } else {
                if item.work6 == 0 {
                    let (dx, dy) = rope_offset(self.tow.angle8());
                    let item = &mut self.objects.slots[slot];
                    item.x = self.pod.body.x.wrapping_add_signed(dx);
                    item.y = self.pod.body.y.wrapping_add_signed(dy);
                }
            }
        }
        self.shield_ring();
        if self.pod.state == STATE_RESPAWN {
            self.respawn_step();
        }
    }

    /// Keeps what hangs on the pod at the pod after a tick's move (positions only; the rest of
    /// their logic runs on the step): the flame in slot 1, the shield ring or the beam in slot
    /// 2, and the towed item on its rope.
    pub(super) fn attach_to_pod(&mut self) {
        let (x, y) = (self.pod.body.x, self.pod.body.y);
        if self.pod.state == STATE_POD
            && self.objects.slots[1].state == state::MOTHERSHIP
            && let Some((_, (dx, dy), ..)) = self.pod.flame_picture()
        {
            let flame = &mut self.objects.slots[1];
            flame.x = x.wrapping_add_signed(dx as i16);
            flame.y = y.wrapping_add_signed(dy as i16);
        }
        if self.tow.holding() {
            let slot = self.tow.slot;
            let item = &mut self.objects.slots[slot];
            if item.state != 0 && item.work6 == 0 {
                let (dx, dy) = rope_offset(self.tow.angle8());
                item.x = x.wrapping_add_signed(dx);
                item.y = y.wrapping_add_signed(dy);
            }
        }
        match self.objects.slots[2].state {
            state::SHIELD_RING if self.pod.state == STATE_POD => {
                let ring = &mut self.objects.slots[2];
                (ring.x, ring.y) = (x, y);
            }
            state::BEAM => self.place_beam(),
            _ => {}
        }
    }

    /// Slot 2 as the shield ring (player.md 4.7a): made afresh at the pod in every pod step
    /// while the shield is on, freed when it goes off.
    fn shield_ring(&mut self) {
        let ring = &mut self.objects.slots[2];
        if !self.pod.shield {
            if ring.state == state::SHIELD_RING {
                ring.state = 0;
            }
            return;
        }
        if self.pod.state != STATE_POD {
            return;
        }
        let shift = if self.pod.equipment.device == 8 { 2 } else { 1 };
        ring.state = state::SHIELD_RING;
        (ring.x, ring.y) = (self.pod.body.x, self.pod.body.y);
        (ring.flags_x, ring.flags_y) = (0, 0);
        ring.frame = 0x69 + ((self.counter as u8 >> shift) & 3);
    }

    /// Slot 2 as the beam in the object sweep (player.md 8.4): freed when nothing is held or
    /// the item is being taken in, else at the middle of the rope, with its frame and mirror
    /// from the rope's angle (the status-bar split's job).
    pub(super) fn place_beam(&mut self) {
        if self.objects.slots[2].state != state::BEAM {
            return;
        }
        let item = self.objects.slots[self.tow.slot];
        if !self.tow.holding() || item.state == 0 || item.work6 != 0 {
            self.objects.slots[2].state = 0;
            return;
        }
        // Half the distance, truncated toward zero.
        let half = |from: u16, to: u16| {
            let d = i32::from(to.wrapping_sub(from) as i16);
            from.wrapping_add_signed((d.signum() * (d.abs() >> 1)) as i16)
        };
        let angle = self.tow.angle8();
        let beam = &mut self.objects.slots[2];
        beam.x = half(self.pod.body.x, item.x);
        beam.y = half(self.pod.body.y, item.y);
        if !self.beam_caught {
            beam.frame = beam_frame(angle);
            beam.flags_x = (angle >> 5) & 1;
        }
    }

    /// The image of the beam's tile $CF for the rope's angle while the beam shows (player.md
    /// 8.4): image i for i up to 8, else 16 - i.
    pub(super) fn beam_image<'a>(
        &self,
        graphics: &'a crate::game::graphics::LevelGraphics,
    ) -> Option<&'a [u8]> {
        if self.objects.slots[2].state != state::BEAM {
            return None;
        }
        let i = usize::from(((self.tow.angle8() + 4) & 63) >> 2);
        let k = if i <= 8 { i } else { 16 - i };
        graphics.beam_tiles.get(16 * k..16 * k + 16)
    }

    /// The pod sound, $3E on both pulses (player.md 6.3, 7.1, 7.2).
    fn pod_sound(&mut self) {
        self.sfx.push(Request::a(Channel::Pulse1, 0x3E));
        self.sfx.push(Request::b(Channel::Pulse2, 0x3E));
    }

    /// The explosion in slot 1: the pod's or Jetman's (player.md 7.1).
    /// The large explosion at the player (player.md 7.4): only the position, the state, the
    /// sequence position and the life are written; the flags and the rest stay as the flame
    /// left them.
    fn explode_player(&mut self) {
        let slot = &mut self.objects.slots[1];
        (slot.x, slot.y) = (self.pod.body.x, self.pod.body.y);
        slot.state = state::EXPLOSION_LARGE;
        slot.kind = 11;
        slot.index = 1;
    }

    /// Jetman's flame routine (player.md 4.8, "Jetman"; 7.4): it acts on slot 1 whatever the
    /// slot holds. With B held it makes the small flame there; otherwise a live slot is moved
    /// to the flame's place and shown as the flame's frame, and every step with the frame
    /// counter AND 7 = 0 its work byte rises, freeing the slot when it reaches 3.
    fn jetman_flame(&mut self, b_held: bool, c: u8) {
        let fx = self.pod.direction_flags();
        let right = if fx & 4 != 0 { fx >> 1 & 1 } else { fx & 1 };
        let x = if right != 0 {
            self.pod.body.x.wrapping_add(5)
        } else {
            self.pod.body.x.wrapping_sub(5)
        };
        let y = self.pod.body.y.wrapping_add(5);
        let d = self.pod.flame.unwrap_or(1);
        let slot = &mut self.objects.slots[1];
        if b_held {
            slot.state = state::MOTHERSHIP;
            slot.kind = d;
        } else if slot.state == 0 {
            return;
        } else if c & 7 == 0 {
            slot.kind = slot.kind.wrapping_add(1);
            if slot.kind >= 3 {
                slot.state = 0;
                slot.kind = 0;
            }
        }
        (slot.x, slot.y) = (x, y);
        (slot.flags_x, slot.flags_y) = (right, 0);
        slot.frame = 0x10u8.wrapping_add(slot.kind);
    }

    /// The crystal on the beam is spun (economy.md 2.6), counted each tick on the rope's angle
    /// as the step began (`docs/scaled.md`).
    pub(super) fn count_spin(&mut self, angle: u8) {
        let slot = self.tow.slot;
        if !self.tow.holding()
            || self.objects.slots[slot].kind != 15
            || self.objects.slots[slot].work6 != 0
        {
            return;
        }
        match self.tow.count_spin(angle) {
            Some(SpinReward::Points(points)) => self.score.add(points),
            Some(SpinReward::Bonus) => {
                self.lives = self.lives.saturating_add(1);
                self.jetpods = self.jetpods.wrapping_add(1);
                let item = &mut self.objects.slots[slot];
                item.kind = 16;
                item.frame = 0x66;
                let index = usize::from(item.index);
                if let Some(entry) = self.objects.items.get_mut(index) {
                    entry.kind = 16;
                }
            }
            None => {}
        }
    }

    // ---- The beam against the items (A pass item 11) -------------------------------------------

    /// Collisions of the player and the tractor beam with the items (game-flow.md 1.2 item 11).
    pub(super) fn beam(&mut self, world: &World, held: Buttons) {
        let level = &world.levels[self.level];
        let planet = usize::from(level.number) < PLANETS;
        let pod_state = self.pod.state;
        if !matches!(pod_state, 1..=3) {
            return;
        }
        let (px, py) = (self.pod.body.x, self.pod.body.y);
        if planet {
            self.bay_and_dock(level);
        }
        if self.transition.is_some() {
            return;
        }
        // Stars and boxes are collected by touch (13 px on both axes), held load or not.
        for slot in ITEM_SLOTS {
            let item = self.objects.slots[slot];
            if matches!(item.state, state::BONUS_STAR | state::BONUS_BOX)
                && i32::from(relative(item.x, px)).abs() <= 13
                && i32::from(relative(item.y, py)).abs() <= 13
            {
                self.objects.slots[slot].state = 0;
                super::collide::pick_up(
                    &mut self.objects,
                    &mut self.pod,
                    &world.objects,
                    &mut self.score,
                    &mut self.bonus_tag,
                    item,
                );
            }
        }
        if self.tow.holding() {
            self.deliveries(world, level, planet);
            return;
        }
        for slot in ITEM_SLOTS {
            let item = self.objects.slots[slot];
            if item.state < state::ITEM
                || matches!(item.state, state::BONUS_STAR | state::BONUS_BOX)
            {
                continue;
            }
            let dx = i32::from(relative(item.x, px));
            let dy = i32::from(relative(item.y, py));
            let kind = item.kind;
            // Caught by the beam (player.md 8.1).
            let in_reach =
                dx.abs() < 32 && dy.abs() <= i32::from(ROPE[32 + dx.unsigned_abs() as usize]);
            if in_reach
                && pod_state == STATE_POD
                && !self.pod.shield
                && !held.has(Buttons::DOWN)
                && !level.is_warp_level()
                && item.work6 == 0
                && towable(kind, self.pod.equipment.owned & 3)
            {
                self.tow.catch(slot, kind, dx, dy);
                // Player.md 8.1: the catch sounds $12 and $14.
                self.sfx.push(Request::a(Channel::Pulse1, 0x12));
                self.sfx.push(Request::b(Channel::Pulse2, 0x14));
                // The beam's frame byte, which the lift reads, is the angle's from the B pass
                // on; the catch's own picture is frame $38 at the pod (player.md 8.4).
                let beam = &mut self.objects.slots[2];
                beam.frame = beam_frame(self.tow.angle8());
                beam.state = state::BEAM;
                (beam.x, beam.y) = (self.pod.body.x, self.pod.body.y);
                self.beam_caught = true;
                return;
            }
            // Touched (13 px on both axes).
            if dx.abs() > 13 || dy.abs() > 13 {
                continue;
            }
            match kind {
                1 if matches!(pod_state, 2 | 3) => {
                    // A spare pod: Jetman assembles it (player.md 6.3).
                    self.objects.slots[slot].state = 0;
                    if let Some(entry) = self.objects.items.get_mut(usize::from(item.index)) {
                        entry.kind = 0;
                    }
                    self.pod.find_spare_pod();
                    return;
                }
                18 if !self.pod.shield => {
                    // The wormhole: the gate state, or the bounce of a closed one (player.md
                    // 9.2.1, 9.2.2).
                    self.touch_wormhole(world, level, slot);
                    return;
                }
                16 if level.is_warp_level()
                    && (matches!(pod_state, 2 | 3)
                        || (pod_state == STATE_POD && !self.pod.shield)) =>
                {
                    // A crystal of a warp level is collected by touch (economy.md 5.4): counted,
                    // sounding $1C on both pulses (sound.md 8.3).
                    self.warp_crystals = self.warp_crystals.wrapping_add(1);
                    self.sfx.push(Request::a(Channel::Pulse1, 0x1C));
                    self.sfx.push(Request::b(Channel::Pulse2, 0x1C));
                    // A burst of three rising sparks at the crystal (economy.md 5.4).
                    let at = (item.x, item.y);
                    self.objects.make_sparks_as(
                        &world.objects,
                        &mut self.rng,
                        &[(0, 0x20, at)],
                        state::SPARK_RISING,
                    );
                    self.use_up(slot);
                    return;
                }
                26 => {
                    self.request(Transition::Restart);
                    return;
                }
                20 if pod_state == STATE_POD && !self.pod.shield => {
                    self.request(Transition::Leave(item.index));
                    return;
                }
                27 if pod_state == STATE_POD && !self.pod.shield => {
                    // Used up by the touch (objects.md 10.2).
                    self.objects.slots[slot].state = 0;
                    if let Some(entry) = self.objects.items.get_mut(usize::from(item.index)) {
                        entry.kind = 0;
                    }
                    self.request(Transition::Interlude);
                    return;
                }
                _ => {}
            }
        }
    }

    /// The per-level routine of the B pass (levels.md 6.6): after 1,280 frames the eggs of
    /// levels 0 and 3 that were not met never spawn, and after 512 the special ring of level 1
    /// is removed.
    pub(super) fn level_routine(&mut self, world: &World, level: &Level) {
        let high = (self.counter >> 8) as u8;
        match level.number {
            0 | 3 if high >= 5 => {
                let index = if level.number == 0 { 0 } else { 70 };
                if let Some(entry) = self.objects.present.get_mut(index) {
                    *entry = 0;
                }
            }
            1 if high >= 2 => {
                if let Some(entry) = self.objects.items.get_mut(1) {
                    entry.kind = 0;
                }
            }
            super::final_level::FINAL_LEVEL => {
                let mut env = super::objects::Env {
                    data: &world.objects,
                    level,
                    camera: self.camera.position,
                    pod: &mut self.pod,
                    rng: &mut self.rng,
                    counter: self.counter,
                    score: &mut self.score,
                    bonus_tag: self.bonus_tag,
                    beam_spin: None,
                    beam_slot: 0,
                };
                if self.objects.final_routine(
                    &mut env,
                    &mut self.effect,
                    self.route.done,
                    self.counter,
                ) && !self.finished
                {
                    self.finished = true;
                    self.request(Transition::Ending);
                }
            }
            _ => {}
        }
    }

    /// The mothership's fuel count: the kind of items list entry 0 on a planet (economy.md 3.4).
    pub(super) fn fuel_count(&self) -> u8 {
        self.objects.items.first().map_or(0, |entry| entry.kind)
    }

    /// Sets the fuel count and the picture of the mothership's indicator (economy.md 3.4).
    fn set_fuel_count(&mut self, count: u8) {
        if let Some(entry) = self.objects.items.first_mut() {
            entry.kind = count;
        }
        for slot in ITEM_SLOTS {
            let o = &mut self.objects.slots[slot];
            if o.state == state::MOTHERSHIP && o.index == 0 {
                o.kind = count;
                o.frame = count.wrapping_add(0x74);
            }
        }
    }

    /// The bay area flag and the dock strip (economy.md 2.2).
    fn bay_and_dock(&mut self, level: &Level) {
        let dx = i32::from(relative(self.pod.body.x, level.home.x));
        let dy = i32::from(relative(self.pod.body.y, level.home.y));
        let holding = self.tow.holding();
        let in_x = (-13..=12).contains(&dx);
        let strip = in_x && (0..=7).contains(&dy);
        if !holding && strip {
            self.request(Transition::Dock);
        }
        if in_x && (-16..=39).contains(&dy) {
            if !(holding && strip) {
                self.pod.bay = true;
            }
        } else {
            self.pod.bay = false;
        }
    }

    /// The held item against the mothership and the rings (economy.md 2.2).
    fn deliveries(&mut self, world: &World, level: &Level, planet: bool) {
        let slot = self.tow.slot;
        let item = self.objects.slots[slot];
        if !is_delivery(item.kind) {
            return;
        }
        if planet {
            let dx = i32::from(relative(item.x, level.home.x));
            let dy = i32::from(relative(item.y, level.home.y));
            if dx.abs() <= 7 && (-6..=31).contains(&dy) {
                self.deliver(world, slot, true);
                return;
            }
            if dx.abs() <= 7 && (-208..=-7).contains(&dy) {
                // Above the bay: snapped to the column and lowered at 1.00 px a step.
                let item = &mut self.objects.slots[slot];
                item.x = level.home.x;
                item.work6 = 1;
            }
        }
        // Through a transporter ring: within 15 px on both axes.
        for ring in ITEM_SLOTS {
            let r = self.objects.slots[ring];
            if r.state != state::RING || ring == slot || r.work6 != 0 {
                continue;
            }
            let dx = i32::from(relative(item.x, r.x));
            let dy = i32::from(relative(item.y, r.y));
            if dx.abs() <= 15 && dy.abs() <= 15 {
                self.deliver(world, slot, false);
                return;
            }
        }
    }

    /// An item is pulled into the mothership (economy.md 2.3). `at_mothership` is false for a
    /// delivery through a ring.
    fn deliver(&mut self, world: &World, slot: usize, at_mothership: bool) {
        let item = self.objects.slots[slot];
        let kind = item.kind;
        let level = &world.levels[self.level];
        let mut delivery = Delivery {
            kind,
            slot,
            index: item.index,
            message_kind: 1,
            text: 6,
            cargo: 0,
        };
        match kind {
            3 => {
                self.pod.equipment.owned |= 0x80;
                delivery.text = 7;
            }
            4 => {
                self.pod.equipment.owned |= 0x40;
                delivery.text = 8;
            }
            5 => delivery.text = 45,
            6 => {
                self.mapping_device = true;
                delivery.text = 9;
            }
            9 => {
                delivery.cargo = ((u16::from(item.index) + u16::from(level.number)) % 17) as u8;
                delivery.message_kind = 3;
                delivery.text = 27;
            }
            10 => {
                let count = self.fuel_count();
                if at_mothership && count < 25 {
                    // A cell taken into the mothership sounds $3C on both pulses (Mesen, seen
                    // as the count reached 25).
                    self.sfx.push(Request::a(Channel::Pulse1, 0x3C));
                    self.sfx.push(Request::b(Channel::Pulse2, 0x3C));
                }
                if at_mothership && count < 24 {
                    // A cell for the tank, no message (economy.md 3.4).
                    self.set_fuel_count(count + 1);
                    self.use_up(slot);
                    return;
                }
                if at_mothership && count == 25 {
                    self.use_up(slot);
                    return;
                }
                let next = (count + 1).min(25);
                if next != count {
                    self.set_fuel_count(next);
                }
                delivery.text = if next == 25 { 22 } else { 21 };
            }
            12 => {
                delivery.message_kind = 4;
                delivery.text = 46;
            }
            15 | 16 => {
                delivery.message_kind = 2;
                if self.crystals < 6 {
                    self.crystals += 1;
                    self.score.add(500);
                    delivery.text = 25;
                } else {
                    self.crystals = 0;
                    self.score.add(2000);
                    self.lives = self.lives.saturating_add(1);
                    self.jetpods = self.jetpods.wrapping_add(1);
                    delivery.text = 50;
                }
            }
            19 => {
                // The planet's flag (economy.md 5.1); the planet of a second level is level - 13,
                // and planet 12 sets planet 10's flag (game-flow.md 7.4, entry to the final level).
                self.planet_flags |= 1 << super::gate::planet_flag_bit(usize::from(level.number));
                delivery.message_kind = 5;
                delivery.text = if self.planet_flags.count_ones() >= 12 {
                    48
                } else {
                    47
                };
            }
            // Kinds no list uses: only the message (economy.md 2.3, "text 6, then kind + 6").
            7 | 8 | 13 | 14 => delivery.text = kind + 6,
            _ => {}
        }
        self.request(Transition::Delivery(delivery));
    }

    /// The item is used up: released, its list entry cleared and its slot freed.
    fn use_up(&mut self, slot: usize) {
        let index = usize::from(self.objects.slots[slot].index);
        if let Some(entry) = self.objects.items.get_mut(index) {
            entry.kind = 0;
        }
        self.objects.slots[slot].state = 0;
        self.tow.slot = 0;
    }

    // ---- Acting on a scene change ------------------------------------------------------------

    /// Acts on the scene change that was requested: 64 frames into a delivery's message the value
    /// is added and the item goes (economy.md 2.3), then play goes on as the caller's scenes
    /// decide. The scenes between (the message, the surface menu, game over) are not here.
    pub fn resume(&mut self, world: &World) -> Outcome {
        let Some(transition) = self.transition.take() else {
            return Outcome::Playing;
        };
        match transition {
            Transition::Delivery(d) => {
                self.score.add(kind_value(d.kind, d.cargo));
                self.use_up(d.slot);
                // The warpship is complete (all twelve flags, planet 12 counting as planet 10):
                // after a message of any of the kinds 1-5 the final level follows.
                if self.planet_flags & 0x0FFF == 0x0FFF {
                    return Outcome::Elsewhere(Transition::FinalLevel);
                }
                // After the screen: in the bay area a lone Jetman docks, the pod docks if the
                // planet's flag is set (economy.md 2.3).
                let level = &world.levels[self.level];
                let flagged = self.planet_flags
                    & (1 << super::gate::planet_flag_bit(usize::from(level.number)))
                    != 0;
                if self.pod.bay && (self.pod.state != STATE_POD || flagged) {
                    return self.dock();
                }
                // game-flow.md 7.4, "Return from the message screen": before the level is
                // redrawn the position is rounded down to a multiple of 8 on both axes with
                // the fractions cleared, and the camera is placed from it.
                let body = &mut self.pod.body;
                (body.x, body.y) = (body.x & !7, body.y & !7);
                (body.x_sub, body.y_sub) = (0, 0);
                self.reload(world);
                // Measured: the hit pause counts down once more across the message screen.
                self.pod.hit_pause_countdown();
                Outcome::Playing
            }
            Transition::Restart => {
                // The special ring puts the pod at a fixed place and the level starts again
                // there (economy.md 2.1, Mesen).
                let level = &world.levels[self.level];
                let body = &mut self.pod.body;
                (body.x, body.y) = (1058, 1404);
                (body.x_sub, body.y_sub) = (0, 0);
                // One step of gravity from rest has been applied (measured: 0.02 down, level 0).
                let pull = i16::from(level.gravity);
                body.set_h_speed(0);
                body.set_v_speed(pull.unsigned_abs() / 4);
                body.v_down = pull >= 0;
                body.y_sub = (pull.unsigned_abs() / 4) << 8;
                self.reload(world);
                // The objects of the old place stay until the window frees them; the
                // placement scan starts over from the new camera (measured: its cursor too).
                self.objects.cursor = 0;
                self.objects
                    .start_scan(&world.objects, level, self.camera.position);
                self.objects.item_scan(&world.objects, self.camera.position);
                Outcome::Playing
            }
            Transition::Dock => self.dock(),
            Transition::JetmanLost => {
                // Game-flow 7.4: lives were set to 0 at the kill when only one was left.
                if !self.cheats.infinite_lives {
                    if self.lives == 0 {
                        return Outcome::GameOver;
                    }
                    self.lives -= 1;
                    if self.lives == 0 {
                        return Outcome::GameOver;
                    }
                }
                // Game-flow.md 7.4: effect $32 with the request lock (sound.md 8.2).
                self.sfx.push(Request::a(Channel::Pulse1, 0x32));
                self.sfx.push(Request::Lock(0x81));
                self.begin_respawn(world);
                Outcome::Playing
            }
            other => Outcome::Elsewhere(other),
        }
    }

    /// The points the badge intermission counts for `crystals` crystals: 100 each, 1,100 for the
    /// tenth (economy.md 5.4).
    pub fn badge_points(crystals: u8) -> u32 {
        (1..=u32::from(crystals))
            .map(|n| if n >= 10 { 1100 } else { 100 })
            .sum()
    }

    /// The game on the planet after a warp level and its badge intermission: the crystals are
    /// counted, then code 3 (game-flow.md 7.4, code 6 with a destination of 0).
    pub fn after_badge(&self, world: &World) -> Scaled {
        self.after_badge_counting(world, Self::badge_points(self.warp_crystals))
    }

    /// The final level, entered after the last warpship piece (game-flow.md 7.4, "Entry to the
    /// final level"): no launch scene; lives, jetpods, the fitted weapon and the crystals
    /// stored are 0, and the score, the pod and its equipment stay.
    pub fn enter_final_level(&self, world: &World) -> Scaled {
        let mut seed = self.seed();
        seed.counter = self.counter & !0xFFFF;
        seed.pod_leftovers = self.pod_leftovers();
        (seed.lives, seed.jetpods, seed.weapon, seed.crystals) = (0, 0, 0, 0);
        seed.final_entry = true;
        Scaled::new(world, usize::from(super::final_level::FINAL_LEVEL), seed)
    }

    /// [`after_badge`](Self::after_badge) with the points the badge screen counted.
    pub fn after_badge_counting(&self, world: &World, points: u32) -> Scaled {
        let mut level = self.clone();
        level.score.add(points);
        level.change_level(world)
    }

    /// Back in the level after the backdrop interlude and the badge intermission: the crystals
    /// counted add `points`, the player and the objects are as they were, the level is placed
    /// again (game-flow.md 7.4, code 6 with a saved position).
    pub fn resume_after_bonus(&mut self, world: &World, points: u32) {
        self.score.add(points);
        self.reload(world);
    }

    /// Start on the planet map: play goes on as after a scene change, the player and the
    /// objects exactly as they were (game-flow.md 4, 7.4 "Reload").
    pub fn leave_map(&mut self, world: &World, buttons: super::input::Buttons) {
        self.paused = false;
        self.pause_held = buttons;
        self.reload(world);
    }

    /// Docking (game-flow.md 7.4 code 2): Jetman alone brings a jetpod back.
    fn dock(&mut self) -> Outcome {
        if self.pod.state != STATE_POD && self.pod.state != 5 {
            self.jetpods = self.jetpods.wrapping_add(1);
        }
        Outcome::Docked
    }

    /// The reload that follows a scene change inside a level (game-flow.md 7.4): the player and
    /// the objects stay, the camera is placed again from the player, the counters and the
    /// transition guard start over.
    fn reload(&mut self, world: &World) {
        let level = &world.levels[self.level];
        self.camera = Camera::at_start(level, self.pod.body.x, self.pod.body.y);
        self.objects.item_cursor = 0;
        self.objects.cursor = 0;
        self.objects.item_scan(&world.objects, self.camera.position);
        self.counter &= !0xFFFF;
        self.job = 8;
        self.terrain
            .by_depth(level.number, self.camera.position.y, true);
        // The first controller read after the reload makes nothing newly pressed (measured on a
        // held button: the random generator saw it held and not pressed).
        self.first_read = true;
        // The reload's own pass counts one down (measured, as at the start of a level).
        self.guard = 63;
        // A reload clears RAM $98's palette bits (game-flow.md 9.2), and its set-up runs the
        // drawing stage once, which eases the F gauge (economy.md 3.1).
        self.pod.palette_bits = 0;
        self.pod.ease_fuel_display();
        // Every star's y becomes $FF; the dead mask, the other bytes and the mode stay
        // (graphics.md 5.7.3).
        self.stars.level_load();
        self.pod.width = 6;
        self.pod.half_height = 6;
        if usize::from(level.number) < PLANETS {
            self.last_transition = 2;
        }
        for o in &mut self.objects.slots {
            if o.state == state::MARKER {
                o.state = 0;
            }
        }
        let view = super::View {
            camera: (
                f32::from(self.camera.position.x),
                f32::from(self.camera.position.y),
            ),
            pod: super::pod_position(&self.pod),
            objects: super::object_views(&self.objects),
        };
        self.shown_before = view;
        self.shown_now = view;
        self.step_from = super::places(&self.objects);
        self.step_at = self.clock.time();
    }

    /// Transition 4 with lives left (player.md 7.3): a new pod materialises where Jetman died.
    fn begin_respawn(&mut self, world: &World) {
        for slot in SHOT_SLOTS {
            // Only the state is cleared: the rest stays for the next occupant (measured).
            self.objects.slots[slot].state = 0;
        }
        self.pod.materialised = false;
        self.pod.state = STATE_RESPAWN;
        self.pod.timer = 7;
        self.pod.facing = 0xFF;
        self.reload(world);
    }

    /// The new pod materialising (state 7, player.md 7.3).
    fn respawn_step(&mut self) {
        let c = self.counter as u8;
        if c >= 144 {
            self.finish_respawn();
            return;
        }
        if self.pod.timer & 0x80 == 0 {
            let slot = usize::from((c >> 3) & 7) + 3;
            if self.objects.slots[slot].state == 0 {
                // Spec gap: the particle keeps the hit points and fractions its slot's last
                // occupant left, as the shots and sparks do.
                let old = self.objects.slots[slot];
                self.objects.slots[slot] = Object {
                    state: state::MOTHERSHIP,
                    x: self.pod.body.x,
                    y: self.pod.body.y,
                    frame: 0x37,
                    hit_points: old.hit_points,
                    x_fraction: old.x_fraction,
                    y_fraction: old.y_fraction,
                    index: ((slot - 3) * 8) as u8,
                    speed_y: 112,
                    ..Object::default()
                };
                self.pod.timer = self.pod.timer.wrapping_sub(1);
            }
        } else {
            if self.pod.materialised {
                self.pod.facing = if c & 6 == 0 { 0xFF } else { 0 };
            }
            if self.pod.timer == 0xF7 {
                self.finish_respawn();
            }
        }
    }

    fn finish_respawn(&mut self) {
        self.pod.state = STATE_POD;
        self.pod.facing = 0;
        self.pod.fuel = super::pod::FULL_FUEL;
        self.pod.width = 6;
        self.pod.half_height = 6;
    }
}
