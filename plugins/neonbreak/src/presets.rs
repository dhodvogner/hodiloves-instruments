//! Factory presets are kits: drum and effect settings. They never touch the pattern slots, so
//! trying another kit keeps the beats you have programmed.
//!
//! A preset only lists what differs from the default kit; loading one resets every sound
//! parameter first, so nothing is inherited from the previous kit. Preset *files* (SAVE / LOAD)
//! hold more: all parameters, the twelve pattern slots and the sample file paths.
//!
//! Plain values: booleans 0 / 1, tune in semitones, pan -1..1, everything else 0..1.
//! `rev_gate`: 0 open, 1 1/32, 2 1/16, 3 1/8, 4 1/4, 5 1/2. Drum prefixes: `k` kick, `s` snare,
//! `c` clap, `hc` closed hat, `ho` open hat, `th` high tom, `tl` low tom, `cy` cymbal.

use crate::bank::{Grid, Slot};
use crate::params::NeonParams;
use nih_plug::prelude::*;

pub struct Preset {
    pub name: &'static str,
    pub category: &'static str,
    /// The library pattern this kit was tuned with (its level is tested with it).
    pub pattern: &'static str,
    pub values: &'static [(&'static str, f32)],
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "Init Kit", category: "Init", pattern: "Two-Step", values: &[] },
    // ------------------------------------------------------------------ fusion: 80s kit, DnB speed
    Preset {
        name: "Neon Break",
        category: "Fusion",
        pattern: "Tom Runner",
        values: &[
            ("gain", 0.60),
            ("s_send", 0.8), ("s_b", 0.5), ("s_dec", 0.5),
            ("th_a", 0.75), ("tl_a", 0.75), ("th_send", 0.6), ("tl_send", 0.6),
            ("rev_size", 0.7), ("rev_gate", 3.0), ("rev_lvl", 0.6),
            ("bus_punch", 0.4), ("bus_glue", 0.4),
        ],
    },
    Preset {
        name: "Retro Roller",
        category: "Fusion",
        pattern: "Roller",
        values: &[
            ("k_a", 0.65), ("k_dec", 0.35),
            ("s_send", 0.7), ("s_a", 0.7), ("s_tone", 0.6),
            ("c_lvl", 0.0),
            ("rev_gate", 2.0), ("rev_size", 0.75), ("rev_lvl", 0.6),
            ("bus_drive", 0.3), ("bus_glue", 0.45),
        ],
    },
    Preset {
        name: "Gated Amen",
        category: "Fusion",
        pattern: "Amen Ghosts",
        values: &[
            ("gain", 0.60),
            ("k_a", 0.7), ("k_b", 0.5), ("k_dec", 0.3),
            ("s_send", 0.75), ("s_dec", 0.4), ("s_tune", 2.0),
            ("cy_lvl", 0.4), ("cy_dec", 0.2), ("cy_b", 0.6), ("cy_tone", 0.7),
            ("rev_gate", 2.0), ("rev_size", 0.8), ("rev_lvl", 0.55),
            ("bus_punch", 0.5), ("bus_drive", 0.4), ("bus_glue", 0.5),
        ],
    },
    Preset {
        name: "Neon Chase",
        category: "Fusion",
        pattern: "Neon Chase",
        values: &[
            ("gain", 0.58),
            ("s_send", 0.85), ("s_b", 0.6), ("c_send", 0.7), ("c_lvl", 0.5),
            ("th_a", 0.8), ("tl_a", 0.8), ("th_send", 0.7), ("tl_send", 0.7),
            ("rev_gate", 3.0), ("rev_size", 0.8), ("rev_lvl", 0.65),
            ("bus_duck", 0.3), ("bus_glue", 0.4),
        ],
    },
    Preset {
        name: "Half-Time Drop",
        category: "Fusion",
        pattern: "Roller",
        values: &[
            ("gain", 0.58),
            ("k_dec", 0.65), ("k_tune", -2.0), ("k_tone", 0.45),
            ("s_send", 0.9), ("s_dec", 0.6), ("s_tune", -2.0),
            ("rev_gate", 3.0), ("rev_size", 0.85), ("rev_lvl", 0.7),
            ("bus_punch", 0.45), ("bus_glue", 0.4),
        ],
    },
    // ------------------------------------------------------------------ synthwave (half-time)
    Preset {
        name: "Miami Nights",
        category: "Synthwave",
        pattern: "Half Backbeat",
        values: &[
            ("k_dec", 0.55), ("k_a", 0.4), ("k_b", 0.2),
            ("s_send", 0.9), ("s_dec", 0.55), ("s_b", 0.45), ("s_tune", -1.0),
            ("c_send", 0.7), ("c_lvl", 0.45),
            ("hc_dec", 0.2), ("ho_dec", 0.5),
            ("rev_gate", 3.0), ("rev_size", 0.85), ("rev_tone", 0.7), ("rev_lvl", 0.7),
            ("bus_glue", 0.35), ("bus_drive", 0.15),
        ],
    },
    Preset {
        name: "Outrun Drive",
        category: "Synthwave",
        pattern: "Outrun",
        values: &[
            ("gain", 0.60),
            ("k_dec", 0.5), ("k_a", 0.55), ("k_tone", 0.4),
            ("s_send", 0.8), ("s_a", 0.7), ("c_send", 0.6), ("c_lvl", 0.5),
            ("ho_lvl", 0.4), ("ho_dec", 0.3),
            ("rev_gate", 3.0), ("rev_size", 0.75), ("rev_lvl", 0.6),
            ("bus_duck", 0.45), ("bus_glue", 0.4),
        ],
    },
    Preset {
        name: "Simmons Sunset",
        category: "Synthwave",
        pattern: "Miami Toms",
        values: &[
            ("k_a", 0.6), ("k_dec", 0.5),
            ("s_b", 0.85), ("s_a", 0.45), ("s_send", 0.8), ("s_dec", 0.6),
            ("th_a", 0.9), ("tl_a", 0.9), ("th_dec", 0.65), ("tl_dec", 0.7),
            ("th_send", 0.7), ("tl_send", 0.7), ("th_tone", 0.15), ("tl_tone", 0.15),
            ("rev_gate", 4.0), ("rev_size", 0.8), ("rev_lvl", 0.65),
        ],
    },
    Preset {
        name: "Linn Tight",
        category: "Synthwave",
        pattern: "Half Backbeat",
        values: &[
            ("gain", 0.78),
            ("k_dec", 0.3), ("k_a", 0.35), ("k_b", 0.45), ("k_tune", 3.0),
            ("s_dec", 0.3), ("s_a", 0.5), ("s_b", 0.15), ("s_send", 0.3), ("s_tune", 2.0),
            ("c_send", 0.3), ("hc_dec", 0.15), ("hc_a", 0.4),
            ("rev_gate", 2.0), ("rev_size", 0.4), ("rev_lvl", 0.35),
            ("bus_punch", 0.5), ("bus_drive", 0.1), ("bus_glue", 0.2),
        ],
    },
    Preset {
        name: "Slow Fade",
        category: "Synthwave",
        pattern: "Breakdown",
        values: &[
            ("gain", 0.85),
            ("k_dec", 0.75), ("k_tune", -3.0), ("k_a", 0.3),
            ("s_send", 1.0), ("s_dec", 0.7), ("s_tune", -3.0),
            ("cy_send", 0.6), ("cy_dec", 0.8), ("cy_lvl", 0.4),
            ("rev_gate", 0.0), ("rev_size", 0.9), ("rev_lvl", 0.6), ("rev_tone", 0.5),
            ("bus_glue", 0.2),
        ],
    },
    // ------------------------------------------------------------------ drum and bass
    Preset {
        name: "Clean Two-Step",
        category: "DnB",
        pattern: "Two-Step",
        values: &[
            ("k_a", 0.7), ("k_dec", 0.3), ("k_b", 0.45), ("k_tune", 2.0),
            ("s_dec", 0.35), ("s_a", 0.65), ("s_tune", 3.0), ("s_send", 0.25),
            ("hc_dec", 0.15),
            ("rev_gate", 0.0), ("rev_size", 0.3), ("rev_lvl", 0.3),
            ("bus_punch", 0.5), ("bus_glue", 0.35),
        ],
    },
    Preset {
        name: "Roller 174",
        category: "DnB",
        pattern: "Roller",
        values: &[
            ("k_a", 0.75), ("k_dec", 0.25), ("k_b", 0.5), ("k_tone", 0.45),
            ("s_dec", 0.3), ("s_a", 0.7), ("s_tune", 4.0), ("s_tone", 0.65), ("s_send", 0.2),
            ("hc_dec", 0.12), ("ho_dec", 0.25),
            ("swing", 0.12),
            ("rev_gate", 0.0), ("rev_size", 0.25), ("rev_lvl", 0.3),
            ("bus_punch", 0.55), ("bus_drive", 0.3), ("bus_glue", 0.45),
        ],
    },
    Preset {
        name: "Amen Smash",
        category: "DnB",
        pattern: "Amen Ghosts",
        values: &[
            ("gain", 0.60),
            ("k_a", 0.6), ("k_dec", 0.3), ("k_tone", 0.6), ("k_b", 0.5),
            ("s_dec", 0.4), ("s_a", 0.55), ("s_tune", 1.0), ("s_send", 0.35),
            ("cy_lvl", 0.45), ("cy_dec", 0.15), ("cy_b", 0.5), ("cy_tone", 0.75),
            ("swing", 0.1), ("human", 0.35),
            ("rev_gate", 0.0), ("rev_size", 0.35), ("rev_lvl", 0.35),
            ("bus_punch", 0.4), ("bus_drive", 0.65), ("bus_glue", 0.6),
        ],
    },
    Preset {
        name: "Jungle Shuffle",
        category: "DnB",
        pattern: "Jungle Shuffle",
        values: &[
            ("gain", 0.62),
            ("k_a", 0.55), ("k_dec", 0.4), ("k_tune", -1.0),
            ("s_dec", 0.4), ("s_tune", 5.0), ("s_a", 0.5), ("s_b", 0.4), ("s_send", 0.3),
            ("hc_a", 0.3), ("hc_dec", 0.2),
            ("swing", 0.25), ("human", 0.4),
            ("rev_gate", 0.0), ("rev_size", 0.4), ("rev_lvl", 0.35),
            ("bus_drive", 0.45), ("bus_glue", 0.5),
        ],
    },
    Preset {
        name: "Neuro Stomp",
        category: "DnB",
        pattern: "Neuro Stomp",
        values: &[
            ("gain", 0.56),
            ("k_a", 0.9), ("k_dec", 0.3), ("k_tone", 0.75), ("k_b", 0.6),
            ("s_dec", 0.35), ("s_a", 0.8), ("s_tone", 0.75), ("s_tune", 2.0), ("s_send", 0.2),
            ("c_lvl", 0.5), ("c_b", 0.7),
            ("hc_a", 1.0), ("hc_b", 0.6), ("hc_dec", 0.1),
            ("rev_gate", 1.0), ("rev_size", 0.5), ("rev_lvl", 0.4),
            ("bus_punch", 0.6), ("bus_drive", 0.6), ("bus_glue", 0.55),
        ],
    },
    Preset {
        name: "Liquid Ride",
        category: "DnB",
        pattern: "Liquid",
        values: &[
            ("k_a", 0.5), ("k_dec", 0.4), ("k_b", 0.2),
            ("s_dec", 0.45), ("s_a", 0.5), ("s_tune", 2.0), ("s_send", 0.45),
            ("hc_lvl", 0.45), ("hc_dec", 0.2),
            ("cy_lvl", 0.35), ("cy_dec", 0.3), ("cy_b", 0.7), ("cy_a", 0.5),
            ("human", 0.3),
            ("rev_gate", 0.0), ("rev_size", 0.6), ("rev_tone", 0.5), ("rev_lvl", 0.4),
            ("bus_punch", 0.3), ("bus_drive", 0.1), ("bus_glue", 0.3),
        ],
    },
    Preset {
        name: "Dancefloor Banger",
        category: "DnB",
        pattern: "Dancefloor",
        values: &[
            ("gain", 0.60),
            ("k_a", 0.8), ("k_dec", 0.35), ("k_b", 0.5), ("k_tone", 0.5),
            ("s_dec", 0.4), ("s_a", 0.7), ("s_tune", 2.0), ("s_send", 0.4),
            ("c_lvl", 0.55), ("c_send", 0.4),
            ("rev_gate", 2.0), ("rev_size", 0.6), ("rev_lvl", 0.45),
            ("bus_punch", 0.5), ("bus_drive", 0.35), ("bus_glue", 0.5), ("bus_duck", 0.35),
        ],
    },
    Preset {
        name: "Ghost Train",
        category: "DnB",
        pattern: "Ghost Train",
        values: &[
            ("k_a", 0.65), ("k_dec", 0.3),
            ("s_dec", 0.35), ("s_a", 0.6), ("s_tune", 3.0), ("s_send", 0.3),
            ("hc_dec", 0.12), ("hc_lvl", 0.5), ("ho_dec", 0.3),
            ("swing", 0.15), ("human", 0.45),
            ("rev_gate", 0.0), ("rev_size", 0.35), ("rev_lvl", 0.3),
            ("bus_punch", 0.45), ("bus_drive", 0.3), ("bus_glue", 0.45),
        ],
    },
    Preset {
        name: "Steppa",
        category: "DnB",
        pattern: "Steppa",
        values: &[
            ("gain", 0.54),
            ("k_a", 0.7), ("k_dec", 0.4), ("k_tone", 0.55),
            ("s_dec", 0.4), ("s_a", 0.65), ("s_send", 0.35),
            ("hc_dec", 0.3), ("hc_a", 0.5),
            ("rev_gate", 2.0), ("rev_size", 0.5), ("rev_lvl", 0.4),
            ("bus_drive", 0.4), ("bus_glue", 0.5), ("bus_duck", 0.4),
        ],
    },
];

/// Settings a kit preset leaves alone: they belong to the project, not to the drum sound.
const KEPT: [&str; 5] = ["mode", "slot", "kick_out", "bpm", "fill"];

/// Load a kit: reset every sound parameter to its default, then apply the preset's values. The
/// pattern slots, the pattern mode and the kick routing are not touched.
pub fn apply(params: &NeonParams, setter: &ParamSetter, preset: &Preset) {
    for (id, ptr, _group) in params.param_map() {
        if KEPT.contains(&id.as_str()) {
            continue;
        }
        // SAFETY: the pointers come from `params`, which outlives this call.
        unsafe {
            let normalized = match preset.values.iter().find(|(k, _)| *k == id) {
                Some((_, plain)) => ptr.preview_normalized(*plain),
                None => ptr.default_normalized_value(),
            };
            setter.raw_context.raw_begin_set_parameter(ptr);
            setter.raw_context.raw_set_parameter_normalized(ptr, normalized);
            setter.raw_context.raw_end_set_parameter(ptr);
        }
    }
}

/// What a preset file stores besides the parameters: the pattern bank and the sample paths.
pub fn extra_json(params: &NeonParams) -> serde_json::Value {
    serde_json::json!({ "bank": params.bank.data(), "samples": params.samples.paths() })
}

/// Restore the pattern bank and samples from a loaded preset file, where it has them.
pub fn apply_extra(params: &NeonParams, extra: &serde_json::Value) {
    if let Some(data) = extra.get("bank").and_then(crate::bank::data_from_json) {
        params.bank.set_data(&data);
    }
    if let Some(list) = extra.get("samples").and_then(|v| v.as_array()) {
        let paths: Vec<String> = list.iter().map(|p| p.as_str().unwrap_or("").to_string()).collect();
        params.samples.set_paths(&paths);
    }
}

/// One pattern slot as stored in a pattern file.
pub fn slot_json(slot: &Slot) -> serde_json::Value {
    serde_json::json!({ "pattern": slot.grid.to_vec() })
}

/// A pattern slot from a loaded pattern file.
pub fn slot_from_json(name: &str, extra: &serde_json::Value) -> Option<Slot> {
    let data: Vec<f32> = extra.get("pattern")?.as_array()?.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
    Some(Slot { name: name.to_string(), grid: Grid::from_slice(&data) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank::PATTERN_KEY;
    use crate::engine::{Engine, Snap, Transport};
    use crate::seq::pattern_by_name;
    use std::collections::HashSet;

    #[test]
    fn every_preset_only_uses_real_ids_and_in_range_values() {
        let params = NeonParams::default();
        let map = params.param_map();
        let ids: HashSet<&str> = map.iter().map(|(id, _, _)| id.as_str()).collect();
        let mut names = HashSet::new();
        for preset in PRESETS {
            assert!(names.insert(preset.name), "duplicate preset name '{}'", preset.name);
            assert!(pattern_by_name(preset.pattern).is_some(), "preset '{}': no pattern '{}'", preset.name, preset.pattern);
            let mut seen = HashSet::new();
            for (id, plain) in preset.values {
                assert!(ids.contains(id), "preset '{}' uses unknown parameter '{id}'", preset.name);
                assert!(!KEPT.contains(id), "preset '{}' sets '{id}', which kits leave alone", preset.name);
                assert!(seen.insert(*id), "preset '{}' sets '{id}' twice", preset.name);
                let ptr = map.iter().find(|(k, _, _)| k == id).unwrap().1;
                let back = unsafe { ptr.preview_plain(ptr.preview_normalized(*plain)) };
                assert!(
                    (back - plain).abs() <= 0.01 * plain.abs().max(1.0),
                    "preset '{}': {id} = {plain} is out of range (clamps to {back})",
                    preset.name
                );
            }
        }
        assert!(PRESETS[0].values.is_empty(), "the first preset must be the default kit");
    }

    #[test]
    fn pattern_and_preset_files_round_trip() {
        let params = NeonParams::default();
        let mut slot = params.bank.slot(3);
        slot.grid.keep_hats = true;
        slot.grid.prob[1][7] = 0.5;
        assert_eq!(slot_from_json(&slot.name, &slot_json(&slot)), Some(slot.clone()));

        params.bank.set_slot(3, slot.clone());
        let extra = extra_json(&params);
        let other = NeonParams::default();
        apply_extra(&other, &extra);
        assert_eq!(other.bank.slot(3), slot);
        // A preset file from before patterns were stored in it changes nothing.
        apply_extra(&other, &serde_json::json!({}));
        assert_eq!(other.bank.slot(3), slot);
    }

    #[test]
    fn every_preset_sounds_at_a_sensible_level() {
        let sr = 48_000.0;
        let mut failures = 0;
        for preset in PRESETS {
            let mut snap = Snap::read(&NeonParams::default());
            let mut gain = nih_plug::util::db_to_gain(-3.0);
            for (id, plain) in preset.values {
                assert!(snap.set(id, *plain));
                if *id == "gain" {
                    gain = *plain;
                }
            }
            // Each kit is checked with the beat it was tuned for, held from its pattern key.
            let mut engine = Engine::new(sr);
            engine.grids_mut()[0] = Grid::from_pattern(pattern_by_name(preset.pattern).unwrap());
            engine.note_on(PATTERN_KEY, 1.0, &snap);
            let n = (4.0 * 4.0 * 60.0 / 174.0 * sr) as usize;
            let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
            let tr = Transport { tempo: 174.0, host_beat: None, internal: false };
            for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
                engine.render(&snap, &tr, cl, cr, None);
            }
            assert!(l.iter().chain(r.iter()).all(|x| x.is_finite()), "preset '{}' produced NaN", preset.name);
            let peak = l.iter().chain(r.iter()).fold(0.0f32, |m, x| m.max(x.abs())) * gain;
            let rms = (l.iter().map(|x| x * x).sum::<f32>() / n as f32).sqrt() * gain;
            println!("{:<20} peak {peak:.2}  rms {rms:.3}", preset.name);
            // Heavily driven kits are squashed on purpose: similar loudness, lower peaks.
            failures += !((0.45..=1.05).contains(&peak) && (0.15..=0.27).contains(&rms)) as usize;
        }
        assert_eq!(failures, 0, "presets outside the level window (peak 0.45..1.05, RMS 0.15..0.27), see the list above");
    }
}
