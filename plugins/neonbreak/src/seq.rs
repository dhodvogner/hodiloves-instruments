//! Pattern library and fills.
//!
//! A pattern is eight rows of sixteen characters, one row per drum in the order kick, snare, clap,
//! closed hat, open hat, high tom, low tom, cymbal. Each character is one sixteenth-note step:
//!
//! | char | meaning                     |
//! |------|-----------------------------|
//! | `.`  | rest                        |
//! | `g`  | ghost note (30 %)           |
//! | `x`  | soft hit (60 %)             |
//! | `X`  | normal hit (85 %)           |
//! | `!`  | accent (100 %)              |
//! | `o`  | ghost note, plays half the time |
//! | `?`  | hit (70 %), plays half the time |

use crate::params::{Fill, STEPS, VOICES};

pub const KICK: usize = 0;
pub const SNARE: usize = 1;
pub const CLAP: usize = 2;
pub const HAT_C: usize = 3;
pub const HAT_O: usize = 4;
pub const TOM_HI: usize = 5;
pub const TOM_LO: usize = 6;
pub const CYMBAL: usize = 7;

/// (velocity, probability) of one pattern character; `None` for an invalid character.
pub fn decode(ch: u8) -> Option<(f32, f32)> {
    Some(match ch {
        b'.' => (0.0, 1.0),
        b'g' => (0.3, 1.0),
        b'x' => (0.6, 1.0),
        b'X' => (0.85, 1.0),
        b'!' => (1.0, 1.0),
        b'o' => (0.3, 0.5),
        b'?' => (0.7, 0.5),
        _ => return None,
    })
}

pub struct Pattern {
    pub name: &'static str,
    pub group: &'static str,
    pub rows: [&'static str; VOICES],
}

impl Pattern {
    /// (velocity, probability) of `voice` at `step`.
    pub fn at(&self, voice: usize, step: usize) -> (f32, f32) {
        self.rows[voice].as_bytes().get(step).and_then(|c| decode(*c)).unwrap_or((0.0, 1.0))
    }
}

const NONE: &str = "................";

pub const PATTERNS: &[Pattern] = &[
    // ------------------------------------------------------------------ drum and bass
    Pattern {
        name: "Two-Step",
        group: "DnB",
        rows: ["X.........X.....", "....X.......X...", NONE, "x.x.x.x.x.x.x.x.", NONE, NONE, NONE, NONE],
    },
    Pattern {
        name: "Roller",
        group: "DnB",
        rows: ["X.........X.....", "....X..g.g..X..g", NONE, "x.xxx.xxx.xxx.xx", "......x.......x.", NONE, NONE, NONE],
    },
    Pattern {
        name: "Amen Ghosts",
        group: "DnB",
        rows: ["X.X.......XX....", "....X..g.g..X..g", NONE, NONE, NONE, NONE, NONE, "X.x.X.x.X.x.X.x."],
    },
    Pattern {
        name: "Jungle Shuffle",
        group: "DnB",
        rows: ["X.....X...X.....", "....X..o.g..X.o.", NONE, "XgxgXgxgXgxgXgxg", NONE, NONE, NONE, NONE],
    },
    Pattern {
        name: "Dancefloor",
        group: "DnB",
        rows: ["X.....X...X.....", "....X.......X...", "....X.......X...", "x.x.x.x.x.x.x.x.", "..............x.", NONE, NONE, NONE],
    },
    Pattern {
        name: "Liquid",
        group: "DnB",
        rows: ["X.........X.....", "....X.......X...", NONE, "x.xgx.xgx.xgx.xg", NONE, NONE, NONE, "x...x...x...x..."],
    },
    Pattern {
        name: "Neuro Stomp",
        group: "DnB",
        rows: ["X..X......X..X..", "....X.......X...", "............x...", "x.x.x.x.x.x.x.x.", NONE, NONE, NONE, NONE],
    },
    Pattern {
        name: "Steppa",
        group: "DnB",
        rows: ["X...X...X...X...", "....X.......X...", NONE, "..x...x...x...x.", NONE, NONE, NONE, NONE],
    },
    Pattern {
        name: "Ghost Train",
        group: "DnB",
        rows: ["X.....?...X.....", "....X.go.g..X.go", NONE, "XgxgXgxgXgxgXgxg", "..........x.....", NONE, NONE, NONE],
    },
    // ------------------------------------------------------------------ fusion
    Pattern {
        name: "Tom Runner",
        group: "Fusion",
        rows: ["X.........X.....", "....X.......X...", NONE, "x.x.x.x.x.x.x.x.", NONE, "......x..x......", ".............x.x", NONE],
    },
    Pattern {
        name: "Neon Chase",
        group: "Fusion",
        rows: ["X.....X...X.....", "....!.......!...", "....X.......X...", "x.xxx.xxx.xxx.xx", NONE, ".......x........", "..............xX", NONE],
    },
    // ------------------------------------------------------------------ synthwave (best in half-time)
    Pattern {
        name: "Half Backbeat",
        group: "Synthwave",
        rows: ["X.......X.X.....", "....X.......X...", "....X.......X...", "x.x.x.x.x.x.x.x.", "..............x.", NONE, NONE, NONE],
    },
    Pattern {
        name: "Outrun",
        group: "Synthwave",
        rows: ["X...X...X...X...", "....!.......!...", "....X.......X...", "x.x.x.x.x.x.x.x.", "..x...x...x...x.", NONE, NONE, NONE],
    },
    Pattern {
        name: "Miami Toms",
        group: "Synthwave",
        rows: ["X.......X.......", "....X.......X...", NONE, "x.x.x.x.x.x.x.x.", NONE, "..........x.x...", ".............x.X", NONE],
    },
    Pattern {
        name: "Breakdown",
        group: "Synthwave",
        rows: ["X...............", "........X.......", NONE, "x...x...x...x...", NONE, NONE, NONE, "!..............."],
    },
];

pub fn pattern_by_name(name: &str) -> Option<&'static Pattern> {
    PATTERNS.iter().find(|p| p.name == name)
}

// ---------------------------------------------------------------------------------------------
// Fills
// ---------------------------------------------------------------------------------------------

/// One fill hit: velocity and how many evenly spaced strokes the step is split into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillHit {
    pub vel: f32,
    pub ratchet: u8,
}

/// What `fill` plays on `voice` at `step` (0..16, position inside the bar), if anything.
///
/// A fill replaces the whole pattern while it is held: nothing but the fill's own hits play.
/// Fills get louder and denser towards the end of the bar, so they lead into the next downbeat.
pub fn fill_hit(fill: Fill, voice: usize, step: usize) -> Option<FillHit> {
    let rise = step as f32 / (STEPS - 1) as f32;
    let hit = |vel: f32, ratchet: u8| Some(FillHit { vel: vel.clamp(0.05, 1.0), ratchet });
    match (fill, voice) {
        // Every fill except the build marks the start of the bar with one kick.
        (Fill::SnareRush | Fill::TomFill | Fill::HatRoll, KICK) if step == 0 => hit(0.9, 1),
        (Fill::SnareRush, SNARE) => hit(0.35 + 0.65 * rise, if step >= 12 { 2 } else { 1 }),
        (Fill::TomFill, TOM_HI) => match step {
            0 | 2 | 3 | 8 | 10 => hit(0.7 + 0.2 * rise, 1),
            _ => None,
        },
        (Fill::TomFill, TOM_LO) => match step {
            4 | 6 | 7 | 11 | 12 | 13 => hit(0.7 + 0.2 * rise, 1),
            14 | 15 => hit(0.9 + 0.1 * rise, if step == 15 { 2 } else { 1 }),
            _ => None,
        },
        (Fill::HatRoll, HAT_C) => hit(0.4 + 0.5 * rise, if step >= 12 { 3 } else { 2 }),
        (Fill::Build, KICK) => {
            let every = if step >= 8 { 2 } else { 4 };
            if step % every == 0 {
                hit(0.9, 1)
            } else {
                None
            }
        }
        (Fill::Build, SNARE) => hit(0.25 + 0.75 * rise, if step >= 14 { 4 } else if step >= 8 { 2 } else { 1 }),
        _ => None,
    }
}

/// MIDI notes that hold a fill while pressed, in the order of the `Fill` variants.
pub const FILL_KEYS: [u8; 4] = [60, 62, 64, 65];

pub fn fill_for_note(note: u8) -> Option<Fill> {
    match note {
        60 => Some(Fill::SnareRush),
        62 => Some(Fill::TomFill),
        64 => Some(Fill::HatRoll),
        65 => Some(Fill::Build),
        _ => None,
    }
}

/// Pattern slot played by holding `note` (the twelve keys from `PATTERN_KEY` up).
pub fn slot_for_note(note: u8) -> Option<usize> {
    let first = crate::bank::PATTERN_KEY;
    (first..first + crate::bank::SLOTS as u8).contains(&note).then(|| (note - first) as usize)
}

/// Note name the way FL Studio's piano roll shows it (MIDI note 60 is C5 there).
pub fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    format!("{}{}", NAMES[note as usize % 12], note / 12)
}

/// General MIDI drum notes mapped to the eight drums.
pub fn voice_for_note(note: u8) -> Option<usize> {
    Some(match note {
        35 | 36 => KICK,
        38 | 40 => SNARE,
        37 | 39 => CLAP,
        42 | 44 => HAT_C,
        46 => HAT_O,
        47 | 48 | 50 => TOM_HI,
        41 | 43 | 45 => TOM_LO,
        49 | 51 | 52 | 53 | 55 | 57 | 59 => CYMBAL,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_pattern_row_is_sixteen_valid_steps() {
        let mut names = HashSet::new();
        for p in PATTERNS {
            assert!(names.insert(p.name), "duplicate pattern name '{}'", p.name);
            for (v, row) in p.rows.iter().enumerate() {
                assert_eq!(row.len(), STEPS, "pattern '{}' row {v} has {} steps", p.name, row.len());
                for ch in row.bytes() {
                    assert!(decode(ch).is_some(), "pattern '{}' row {v}: bad character '{}'", p.name, ch as char);
                }
            }
            assert!(p.rows[KICK].bytes().any(|c| c != b'.'), "pattern '{}' has no kick", p.name);
        }
    }

    #[test]
    fn fills_rise_towards_the_end_of_the_bar() {
        let first = fill_hit(Fill::SnareRush, SNARE, 0).unwrap();
        let last = fill_hit(Fill::SnareRush, SNARE, 15).unwrap();
        assert!(last.vel > first.vel && last.ratchet > first.ratchet);
        assert!(fill_hit(Fill::Off, SNARE, 3).is_none());
        assert!(fill_hit(Fill::SnareRush, KICK, 0).is_some() && fill_hit(Fill::SnareRush, KICK, 10).is_none());
    }

    #[test]
    fn general_midi_notes_reach_the_right_drums() {
        assert_eq!(voice_for_note(36), Some(KICK));
        assert_eq!(voice_for_note(38), Some(SNARE));
        assert_eq!(voice_for_note(42), Some(HAT_C));
        assert_eq!(voice_for_note(46), Some(HAT_O));
        assert_eq!(voice_for_note(60), None);
        assert_eq!(fill_for_note(60), Some(Fill::SnareRush));
        assert_eq!(slot_for_note(72), Some(0));
        assert_eq!(slot_for_note(83), Some(11));
        assert_eq!(slot_for_note(84), None);
        assert_eq!(note_name(60), "C5");
        assert_eq!(note_name(36), "C3");
    }

    #[test]
    fn keys_for_drums_fills_and_patterns_do_not_overlap() {
        for note in 0..128u8 {
            let uses = voice_for_note(note).is_some() as u8 + fill_for_note(note).is_some() as u8 + slot_for_note(note).is_some() as u8;
            assert!(uses <= 1, "note {note} has {uses} jobs");
        }
    }
}
