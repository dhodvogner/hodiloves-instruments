//! The pattern bank: twelve pattern slots that are saved with the project (and in preset files).
//!
//! Each slot is played by holding one key of the octave starting at MIDI note 72, so a whole
//! arrangement can be written as long notes in the piano roll: one note per section, each on the
//! key of the pattern that section should play.

use crate::params::{STEPS, VOICES};
use crate::seq::{Pattern, PATTERNS};
use nih_plug::params::persist::PersistentField;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

pub const SLOTS: usize = 12;
/// MIDI note of the first pattern slot. The twelve slots are the twelve keys from here up.
pub const PATTERN_KEY: u8 = 72;

/// The steps of one pattern, in a form the audio thread can copy without allocating.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Grid {
    /// Play at half speed: sixteen steps last two bars.
    pub half: bool,
    /// In half-time, keep both hi-hats at full speed.
    pub keep_hats: bool,
    /// Velocity per drum and step; 0 = off.
    pub vel: [[f32; STEPS]; VOICES],
    /// Chance (0..1) that the step plays each time around.
    pub prob: [[f32; STEPS]; VOICES],
}

impl Grid {
    pub const EMPTY: Grid =
        Grid { half: false, keep_hats: false, vel: [[0.0; STEPS]; VOICES], prob: [[1.0; STEPS]; VOICES] };

    /// A library pattern. The synthwave ones are written for half-time.
    pub fn from_pattern(p: &Pattern) -> Self {
        let mut g = Self::EMPTY;
        g.half = p.group == "Synthwave";
        for v in 0..VOICES {
            for s in 0..STEPS {
                (g.vel[v][s], g.prob[v][s]) = p.at(v, s);
            }
        }
        g
    }

    /// Flat list for saving: half, keep hats, then every velocity, then every chance.
    pub fn to_vec(&self) -> Vec<f32> {
        let mut out = vec![self.half as u8 as f32, self.keep_hats as u8 as f32];
        out.extend(self.vel.iter().flatten());
        out.extend(self.prob.iter().flatten());
        out
    }

    /// Inverse of [`Self::to_vec`]; missing or broken values fall back to an empty step.
    pub fn from_slice(data: &[f32]) -> Self {
        let mut g = Self::EMPTY;
        let get = |i: usize, default: f32| data.get(i).copied().filter(|x| x.is_finite()).unwrap_or(default).clamp(0.0, 1.0);
        g.half = get(0, 0.0) > 0.5;
        g.keep_hats = get(1, 0.0) > 0.5;
        for v in 0..VOICES {
            for s in 0..STEPS {
                g.vel[v][s] = get(2 + v * STEPS + s, 0.0);
                g.prob[v][s] = get(2 + VOICES * STEPS + v * STEPS + s, 1.0);
            }
        }
        g
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Slot {
    pub name: String,
    pub grid: Grid,
}

impl Slot {
    pub fn from_pattern(p: &Pattern) -> Self {
        Self { name: p.name.to_string(), grid: Grid::from_pattern(p) }
    }

    pub fn empty() -> Self {
        Self { name: "Empty".to_string(), grid: Grid::EMPTY }
    }
}

/// The patterns a new instance starts with, one per slot.
const DEFAULT_BANK: [&str; SLOTS] = [
    "Two-Step",
    "Roller",
    "Amen Ghosts",
    "Jungle Shuffle",
    "Dancefloor",
    "Liquid",
    "Neuro Stomp",
    "Tom Runner",
    "Neon Chase",
    "Half Backbeat",
    "Outrun",
    "Breakdown",
];

pub fn default_slots() -> Vec<Slot> {
    DEFAULT_BANK
        .iter()
        .map(|name| PATTERNS.iter().find(|p| p.name == *name).map(Slot::from_pattern).unwrap_or_else(Slot::empty))
        .collect()
}

/// What is written to the project / preset file: (name, flat grid) per slot.
pub type BankData = Vec<(String, Vec<f32>)>;

/// The bank, shared between the editor (which edits it), the host (which saves and restores it)
/// and the audio thread (which copies the grids whenever `version` changes).
pub struct BankCell {
    slots: RwLock<Vec<Slot>>,
    version: AtomicU32,
}

impl Default for BankCell {
    fn default() -> Self {
        Self { slots: RwLock::new(default_slots()), version: AtomicU32::new(1) }
    }
}

impl BankCell {
    pub fn version(&self) -> u32 {
        self.version.load(Ordering::Acquire)
    }

    pub fn slot(&self, index: usize) -> Slot {
        self.slots.read().map(|s| s[index.min(SLOTS - 1)].clone()).unwrap_or_else(|_| Slot::empty())
    }

    pub fn names(&self) -> Vec<String> {
        self.slots.read().map(|s| s.iter().map(|x| x.name.clone()).collect()).unwrap_or_default()
    }

    pub fn set_slot(&self, index: usize, slot: Slot) {
        if let Ok(mut slots) = self.slots.write() {
            slots[index.min(SLOTS - 1)] = slot;
        }
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    /// Copy every grid for the audio thread. Never blocks: returns false if the bank is being
    /// written right now (the caller simply tries again on the next block).
    pub fn try_grids(&self, out: &mut [Grid; SLOTS]) -> bool {
        match self.slots.try_read() {
            Ok(slots) => {
                for (o, s) in out.iter_mut().zip(slots.iter()) {
                    *o = s.grid;
                }
                true
            }
            Err(_) => false,
        }
    }

    pub fn data(&self) -> BankData {
        self.slots.read().map(|s| s.iter().map(|x| (x.name.clone(), x.grid.to_vec())).collect()).unwrap_or_default()
    }

    /// Replace the bank. Slots missing from `data` keep the factory pattern.
    pub fn set_data(&self, data: &BankData) {
        let mut slots = default_slots();
        for (slot, (name, grid)) in slots.iter_mut().zip(data.iter()) {
            *slot = Slot { name: name.clone(), grid: Grid::from_slice(grid) };
        }
        if let Ok(mut current) = self.slots.write() {
            *current = slots;
        }
        self.version.fetch_add(1, Ordering::AcqRel);
    }
}

impl<'a> PersistentField<'a, BankData> for BankCell {
    fn set(&self, new_value: BankData) {
        self.set_data(&new_value);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&BankData) -> R,
    {
        f(&self.data())
    }
}

/// Bank data from the `extra` part of a preset file.
pub fn data_from_json(value: &serde_json::Value) -> Option<BankData> {
    let list = value.as_array()?;
    Some(
        list.iter()
            .filter_map(|entry| {
                let name = entry.get(0)?.as_str()?.to_string();
                let grid = entry.get(1)?.as_array()?.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
                Some((name, grid))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_round_trips_through_its_saved_form() {
        let mut g = Grid::from_pattern(&PATTERNS[1]);
        g.keep_hats = true;
        g.prob[3][5] = 0.25;
        assert_eq!(Grid::from_slice(&g.to_vec()), g);
        // Truncated data must not panic and gives empty steps.
        assert_eq!(Grid::from_slice(&[1.0]).vel, Grid::EMPTY.vel);
    }

    #[test]
    fn bank_starts_full_and_survives_save_and_restore() {
        let bank = BankCell::default();
        assert_eq!(bank.names().len(), SLOTS);
        assert!(bank.names().iter().all(|n| n != "Empty"), "a default slot names a missing pattern");
        assert!(bank.slot(9).grid.half && !bank.slot(0).grid.half);

        let before = bank.version();
        let mut slot = bank.slot(2);
        slot.name = "Mine".into();
        slot.grid.vel[0][3] = 0.5;
        bank.set_slot(2, slot.clone());
        assert!(bank.version() > before);

        let json = serde_json::json!(bank.data());
        let restored = BankCell::default();
        restored.set_data(&data_from_json(&json).unwrap());
        assert_eq!(restored.slot(2), slot);
        let mut grids = [Grid::EMPTY; SLOTS];
        assert!(restored.try_grids(&mut grids));
        assert_eq!(grids[2].vel[0][3], 0.5);
    }
}
