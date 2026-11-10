//! Factory presets and "reset to defaults".
//!
//! A preset only lists what differs from the default patch. Loading one first resets *every*
//! parameter to its default and then applies the overrides, so presets never inherit leftovers
//! from whatever was loaded before. "Reset" is the same operation with no overrides.
//!
//! Plain values: enums use their variant index, booleans 0 / 1, everything else its natural unit
//! (Hz, ms, 0..1, -1..1, semitones). Index tables, in declaration order:
//!
//! - `RateDiv`:  0 bar, 1 1/2, 2 1/2T, 3 1/4., 4 1/4, 5 1/4T, 6 1/8., 7 1/8, 8 1/8T, 9 1/16.,
//!   10 1/16, 11 1/16T, 12 1/32
//! - `LfoShape`: 0 sine, 1 tri, 2 saw up, 3 saw down, 4 square, 5 soft square, 6 half sine, 7 random
//! - `LfoTrig`:  0 note, 1 grid, 2 free
//! - `DistKind`: 0 off, 1 soft, 2 hard, 3 fold, 4 tube, 5 rectify, 6 crush
//! - `StepLen`:  0 1/16, 1 1/8, 2 1/4, 3 1/2, 4 bar
//! - `FilterKind`: 0 low 24, 1 low 12, 2 band 12; `Wave1`: 0 saw, 1 square, 2 pulse, 3 triangle;
//!   `Wave2`: 0 saw, 1 square, 2 sine, 3 triangle

use crate::params::WubParams;
use nih_plug::prelude::*;

pub struct Preset {
    pub name: &'static str,
    pub category: &'static str,
    pub values: &'static [(&'static str, f32)],
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "Init Wub", category: "Init", values: &[] },
    // ---------------------------------------------------------------- classic wobbles
    Preset {
        name: "Classic Wub 1/4",
        category: "Wobble",
        values: &[
            ("l1_div", 4.0), ("l1_cut", 0.8),
            ("flt_cut", 110.0), ("flt_res", 0.55),
            ("o2_lvl", 0.3),
            ("da_drv", 0.5),
            ("tone_ott", 0.25),
        ],
    },
    Preset {
        name: "Half-Time Heavy 1/2",
        category: "Wobble",
        values: &[
            ("l1_div", 1.0), ("l1_cut", 0.9),
            ("flt_cut", 90.0), ("flt_res", 0.6), ("flt_key", 0.5),
            ("sub_lvl", 0.6),
            ("da_drv", 0.6),
            ("amp_rel", 250.0),
            ("tone_ott", 0.3),
        ],
    },
    Preset {
        name: "Triplet Roller 1/8T",
        category: "Wobble",
        values: &[
            ("gain", 0.55),
            ("l1_shape", 1.0), ("l1_div", 8.0), ("l1_cut", 0.75),
            ("flt_cut", 140.0),
            ("da_kind", 4.0), ("da_drv", 0.5),
        ],
    },
    Preset {
        name: "Clean Sub Wobble",
        category: "Wobble",
        values: &[
            ("o1_uni", 1.0), ("o1_lvl", 0.5), ("o2_lvl", 0.0),
            ("sub_lvl", 0.8), ("sub_flt", 1.0),
            ("l1_div", 4.0), ("l1_cut", 0.6), ("l1_smooth", 0.2),
            ("flt_cut", 200.0), ("flt_res", 0.25),
            ("da_drv", 0.2),
        ],
    },
    Preset {
        name: "Deep Dub Sub",
        category: "Wobble",
        values: &[
            ("o1_wave", 3.0), ("o1_uni", 1.0), ("o2_lvl", 0.0),
            ("sub_lvl", 0.9),
            ("l1_div", 4.0), ("l1_cut", 0.5), ("l1_smooth", 0.3),
            ("flt_cut", 180.0), ("flt_res", 0.2),
            ("da_kind", 4.0), ("da_drv", 0.3),
            ("tone_hi", 6000.0),
            ("amp_rel", 200.0),
        ],
    },
    // ---------------------------------------------------------------- dubstep
    Preset {
        name: "Dubstep Brass 1/8",
        category: "Dubstep",
        values: &[
            ("gain", 0.50),
            ("o1_uni", 5.0), ("o1_det", 0.3),
            ("o2_wave", 0.0), ("o2_lvl", 0.5), ("o2_fine", 12.0),
            ("flt_cut", 220.0), ("flt_res", 0.55), ("flt_drv", 0.3), ("flt_env", 0.45),
            ("fenv_atk", 25.0), ("fenv_dec", 180.0), ("fenv_sus", 0.3),
            ("l1_shape", 5.0), ("l1_cut", 0.55),
            ("da_kind", 2.0), ("da_drv", 0.55),
            ("db_kind", 1.0), ("db_drv", 0.25),
            ("fmt_mix", 0.25), ("fmt_morph", 0.2),
            ("tone_lo", 30.0), ("tone_ott", 0.4),
        ],
    },
    Preset {
        name: "Laser Pluck Wub",
        category: "Dubstep",
        values: &[
            ("gain", 0.57),
            ("l1_shape", 3.0), ("l1_div", 4.0), ("l1_cut", 1.0),
            ("flt_cut", 130.0), ("flt_res", 0.6), ("flt_env", 0.3),
            ("fenv_atk", 5.0), ("fenv_dec", 120.0),
            ("amp_rel", 80.0),
            ("da_kind", 2.0), ("da_drv", 0.45),
            ("tone_ott", 0.3),
        ],
    },
    Preset {
        name: "Tearout Saw",
        category: "Dubstep",
        values: &[
            ("gain", 1.15),
            ("o1_uni", 7.0), ("o1_det", 0.45), ("o1_spr", 0.9),
            ("o2_wave", 0.0), ("o2_semi", 12.0), ("o2_lvl", 0.4),
            ("flt_kind", 2.0), ("flt_cut", 400.0), ("flt_res", 0.5), ("flt_drv", 0.4),
            ("l1_shape", 3.0), ("l1_div", 7.0), ("l1_cut", 0.8),
            ("fmt_mix", 0.3), ("fmt_morph", 0.6),
            ("da_kind", 2.0), ("da_drv", 0.65),
            ("db_kind", 3.0), ("db_drv", 0.3), ("db_mix", 0.6),
            ("tone_ott", 0.6),
        ],
    },
    Preset {
        name: "Screech",
        category: "Dubstep",
        values: &[
            ("gain", 0.90),
            ("o1_uni", 3.0), ("o1_det", 0.2),
            ("o2_wave", 2.0), ("o2_semi", 19.0), ("o2_lvl", 0.0), ("o2_fm", 0.6),
            ("flt_kind", 2.0), ("flt_cut", 900.0), ("flt_res", 0.65), ("flt_env", 0.4),
            ("fenv_dec", 400.0), ("fenv_sus", 0.4),
            ("l1_shape", 2.0), ("l1_div", 10.0), ("l1_cut", 0.4), ("l1_fm", 0.4),
            ("sub_lvl", 0.3),
            ("da_kind", 3.0), ("da_drv", 0.5),
            ("db_kind", 1.0), ("db_drv", 0.35),
            ("tone_lo", 60.0), ("tone_ott", 0.7),
        ],
    },
    // ---------------------------------------------------------------- growls / neuro
    Preset {
        name: "Yoi Yoi Growl",
        category: "Growl",
        values: &[
            ("gain", 1.30),
            ("fmt_mix", 1.0), ("fmt_morph", 0.1), ("fmt_q", 0.55),
            ("l1_div", 4.0), ("l1_cut", 0.3), ("l1_fmt", 0.9),
            ("o2_wave", 2.0), ("o2_semi", 12.0), ("o2_fm", 0.25),
            ("l2_fm", 0.5), ("l2_hz", 0.6),
            ("da_kind", 3.0), ("da_drv", 0.5),
            ("db_kind", 1.0), ("db_drv", 0.4),
            ("tone_ott", 0.5),
        ],
    },
    Preset {
        name: "Filthy FM Growl",
        category: "Growl",
        values: &[
            ("o2_wave", 2.0), ("o2_semi", 7.0), ("o2_lvl", 0.0), ("o2_fm", 0.55),
            ("l1_cut", 0.55),
            ("l2_shape", 1.0), ("l2_hz", 0.6), ("l2_trig", 2.0), ("l2_fm", 0.6),
            ("flt_cut", 260.0), ("flt_res", 0.45),
            ("da_kind", 3.0), ("da_drv", 0.55),
            ("db_kind", 6.0), ("db_drv", 0.25),
            ("tone_ott", 0.5),
        ],
    },
    Preset {
        name: "Vowel Wah 1/8",
        category: "Growl",
        values: &[
            ("gain", 1.40),
            ("o1_uni", 5.0), ("o1_det", 0.3),
            ("fmt_mix", 0.8), ("fmt_morph", 0.25), ("fmt_q", 0.6),
            ("l1_shape", 1.0), ("l1_div", 7.0), ("l1_cut", 0.35), ("l1_fmt", 0.7),
            ("flt_cut", 300.0), ("flt_res", 0.35),
            ("da_kind", 4.0), ("da_drv", 0.5),
            ("tone_ott", 0.4),
        ],
    },
    Preset {
        name: "Robot Talk",
        category: "Growl",
        values: &[
            ("gain", 1.85),
            ("fmt_mix", 1.0), ("fmt_q", 0.7),
            ("l1_shape", 7.0), ("l1_div", 10.0), ("l1_cut", 0.2), ("l1_fmt", 1.0), ("l1_smooth", 0.25),
            ("da_kind", 2.0), ("da_drv", 0.4),
            ("tone_ott", 0.3),
        ],
    },
    // ---------------------------------------------------------------- drum and bass
    Preset {
        name: "Reese Neuro",
        category: "DnB",
        values: &[
            ("o1_uni", 7.0), ("o1_det", 0.55), ("o1_spr", 0.8),
            ("o2_wave", 0.0), ("o2_fine", 18.0), ("o2_lvl", 0.6),
            ("sub_lvl", 0.3),
            ("flt_kind", 1.0), ("flt_cut", 600.0), ("flt_res", 0.35),
            ("l1_shape", 1.0), ("l1_sync", 0.0), ("l1_hz", 0.12), ("l1_trig", 2.0), ("l1_cut", 0.5),
            ("l2_shape", 0.0), ("l2_hz", 0.21), ("l2_trig", 2.0), ("l2_fmt", 0.8),
            ("fmt_mix", 0.6), ("fmt_morph", 0.5), ("fmt_q", 0.6),
            ("da_drv", 0.5),
            ("db_kind", 2.0), ("db_drv", 0.35),
            ("tone_ott", 0.4),
        ],
    },
    Preset {
        name: "DnB Chatter 1/16",
        category: "DnB",
        values: &[
            ("l1_shape", 5.0), ("l1_div", 10.0), ("l1_cut", 0.7), ("l1_smooth", 0.15),
            ("flt_cut", 180.0),
            ("o2_lvl", 0.5),
            ("amp_rel", 60.0),
            ("da_kind", 2.0), ("da_drv", 0.45),
            ("tone_ott", 0.3),
        ],
    },
    Preset {
        name: "Foghorn",
        category: "DnB",
        values: &[
            ("gain", 0.57),
            ("legato", 1.0), ("glide", 60.0),
            ("o1_wave", 2.0), ("o1_pwm", 0.3), ("o1_uni", 3.0), ("o1_det", 0.2),
            ("l2_pwm", 0.6), ("l2_hz", 0.3),
            ("flt_cut", 300.0), ("flt_res", 0.4),
            ("l1_shape", 1.0), ("l1_sync", 0.0), ("l1_hz", 0.25), ("l1_trig", 2.0), ("l1_cut", 0.6),
            ("fmt_mix", 0.5), ("fmt_morph", 0.7),
            ("da_kind", 4.0), ("da_drv", 0.6),
            ("db_kind", 1.0), ("db_drv", 0.4),
            ("tone_ott", 0.5),
            ("amp_rel", 200.0),
        ],
    },
    Preset {
        name: "Liquid Roller 1/8T",
        category: "DnB",
        values: &[
            ("o1_wave", 3.0), ("o1_uni", 3.0), ("o1_det", 0.15), ("o2_wave", 2.0), ("o2_lvl", 0.3),
            ("sub_lvl", 0.7),
            ("flt_kind", 1.0), ("flt_cut", 260.0), ("flt_res", 0.3),
            ("l1_div", 8.0), ("l1_cut", 0.45), ("l1_smooth", 0.3),
            ("da_drv", 0.25),
            ("amp_rel", 180.0),
        ],
    },
    Preset {
        name: "Machine Gun 1/32",
        category: "DnB",
        values: &[
            ("o1_uni", 5.0), ("o1_det", 0.3),
            ("l1_shape", 4.0), ("l1_div", 12.0), ("l1_cut", 0.35), ("l1_amp", 0.9), ("l1_smooth", 0.2),
            ("flt_cut", 500.0), ("flt_res", 0.4),
            ("da_kind", 2.0), ("da_drv", 0.5),
            ("amp_rel", 50.0),
            ("tone_ott", 0.4),
        ],
    },
    // ---------------------------------------------------------------- pattern presets
    Preset {
        name: "Wub Wub Rest",
        category: "Pattern",
        values: &[
            ("pat_on", 1.0), ("pat_len", 8.0), ("pat_step", 1.0),
            ("pat_srate_1", 7.0), ("pat_srate_2", 7.0), ("pat_srate_3", 10.0), ("pat_srate_4", 10.0),
            ("pat_srate_5", 7.0), ("pat_srate_6", 7.0), ("pat_srate_7", 10.0), ("pat_srate_8", 10.0),
            ("pat_son_4", 0.0), ("pat_son_8", 0.0),
            ("l1_cut", 0.75),
            ("da_kind", 2.0), ("da_drv", 0.45),
            ("tone_ott", 0.3),
        ],
    },
    Preset {
        name: "Accelerator",
        category: "Pattern",
        values: &[
            ("pat_on", 1.0), ("pat_len", 4.0), ("pat_step", 2.0),
            ("pat_srate_1", 4.0), ("pat_srate_2", 7.0), ("pat_srate_3", 10.0), ("pat_srate_4", 12.0),
            ("l1_cut", 0.75),
            ("flt_res", 0.55),
            ("da_kind", 1.0), ("da_drv", 0.55),
            ("tone_ott", 0.3),
        ],
    },
    Preset {
        name: "Interval Walker",
        category: "Pattern",
        values: &[
            ("gain", 0.55),
            ("pat_on", 1.0), ("pat_len", 8.0), ("pat_step", 1.0),
            ("pat_ssemi_4", 12.0), ("pat_ssemi_6", -5.0), ("pat_ssemi_8", 7.0),
            ("pat_srate_2", 10.0), ("pat_srate_5", 10.0),
            ("glide", 40.0),
            ("l1_cut", 0.7),
            ("da_kind", 4.0), ("da_drv", 0.5),
        ],
    },
    Preset {
        name: "Dub Skank",
        category: "Pattern",
        values: &[
            ("gain", 0.55),
            ("pat_on", 1.0), ("pat_len", 16.0), ("pat_step", 0.0),
            ("pat_son_2", 0.0), ("pat_son_3", 0.0), ("pat_son_5", 0.0), ("pat_son_6", 0.0), ("pat_son_8", 0.0),
            ("pat_son_10", 0.0), ("pat_son_12", 0.0), ("pat_son_13", 0.0), ("pat_son_15", 0.0), ("pat_son_16", 0.0),
            ("pat_srate_1", 10.0), ("pat_srate_2", 10.0), ("pat_srate_3", 10.0), ("pat_srate_4", 10.0),
            ("pat_srate_5", 10.0), ("pat_srate_6", 10.0), ("pat_srate_7", 10.0), ("pat_srate_8", 10.0),
            ("pat_srate_9", 10.0), ("pat_srate_10", 10.0), ("pat_srate_11", 10.0), ("pat_srate_12", 10.0),
            ("pat_srate_13", 10.0), ("pat_srate_14", 10.0), ("pat_srate_15", 10.0), ("pat_srate_16", 10.0),
            ("l1_shape", 3.0), ("l1_cut", 0.8),
            ("flt_res", 0.5),
            ("amp_rel", 60.0),
            ("da_kind", 2.0), ("da_drv", 0.5),
        ],
    },
    Preset {
        name: "Growl Sentence",
        category: "Pattern",
        values: &[
            ("gain", 1.30),
            ("pat_on", 1.0), ("pat_len", 8.0), ("pat_step", 1.0),
            ("pat_srate_1", 4.0), ("pat_srate_2", 7.0), ("pat_srate_3", 8.0), ("pat_srate_4", 10.0),
            ("pat_srate_5", 4.0), ("pat_srate_6", 10.0), ("pat_srate_7", 7.0), ("pat_srate_8", 12.0),
            ("pat_sdep_3", 0.6), ("pat_sdep_6", 0.5),
            ("pat_ssemi_8", 12.0),
            ("fmt_mix", 0.9), ("fmt_morph", 0.15), ("fmt_q", 0.6),
            ("l1_cut", 0.35), ("l1_fmt", 0.85),
            ("o2_wave", 2.0), ("o2_semi", 12.0), ("o2_fm", 0.3),
            ("da_kind", 3.0), ("da_drv", 0.5),
            ("db_kind", 1.0), ("db_drv", 0.35),
            ("tone_ott", 0.55),
        ],
    },
    // ---------------------------------------------------------------- synthwave / analog
    Preset {
        name: "Outrun Bass",
        category: "Synthwave",
        values: &[
            ("gain", 0.83),
            ("o1_uni", 1.0), ("o2_wave", 1.0), ("o2_lvl", 0.4), ("o2_fine", 6.0),
            ("l1_cut", 0.0),
            ("flt_cut", 300.0), ("flt_res", 0.35), ("flt_env", 0.5),
            ("fenv_atk", 2.0), ("fenv_dec", 160.0), ("fenv_sus", 0.2),
            ("da_drv", 0.25),
            ("amp_dec", 300.0), ("amp_sus", 0.7), ("amp_rel", 100.0),
        ],
    },
    Preset {
        name: "Rolling 16ths",
        category: "Synthwave",
        values: &[
            ("o1_uni", 1.0), ("o2_wave", 1.0), ("o2_lvl", 0.35),
            ("l1_shape", 3.0), ("l1_div", 10.0), ("l1_cut", 0.5), ("l1_smooth", 0.15),
            ("flt_cut", 180.0), ("flt_res", 0.35),
            ("da_drv", 0.3),
            ("amp_rel", 80.0),
        ],
    },
    Preset {
        name: "Analog Pulse Bass",
        category: "Synthwave",
        values: &[
            ("o1_wave", 2.0), ("o1_pwm", 0.35), ("o1_uni", 1.0), ("o2_lvl", 0.0),
            ("l1_cut", 0.0),
            ("l2_pwm", 0.5), ("l2_hz", 0.4),
            ("flt_cut", 380.0), ("flt_res", 0.3), ("flt_env", 0.4),
            ("fenv_atk", 2.0), ("fenv_dec", 220.0), ("fenv_sus", 0.25),
            ("da_kind", 4.0), ("da_drv", 0.3),
            ("amp_rel", 110.0),
        ],
    },
    Preset {
        name: "Retro Wobble 1/8",
        category: "Synthwave",
        values: &[
            ("o1_wave", 1.0), ("o1_uni", 3.0), ("o1_det", 0.15), ("o2_wave", 0.0), ("o2_lvl", 0.3),
            ("l1_shape", 1.0), ("l1_div", 7.0), ("l1_cut", 0.5), ("l1_smooth", 0.3),
            ("flt_cut", 220.0), ("flt_res", 0.3),
            ("da_kind", 4.0), ("da_drv", 0.35),
            ("tone_hi", 8000.0),
        ],
    },
    Preset {
        name: "Retro Octave Run",
        category: "Synthwave",
        values: &[
            ("pat_on", 1.0), ("pat_len", 8.0), ("pat_step", 0.0),
            ("pat_ssemi_3", 12.0), ("pat_ssemi_5", 7.0), ("pat_ssemi_7", 12.0),
            ("pat_srate_1", 10.0), ("pat_srate_2", 10.0), ("pat_srate_3", 10.0), ("pat_srate_4", 10.0),
            ("pat_srate_5", 10.0), ("pat_srate_6", 10.0), ("pat_srate_7", 10.0), ("pat_srate_8", 10.0),
            ("o1_uni", 1.0), ("o2_wave", 1.0), ("o2_lvl", 0.35),
            ("l1_shape", 3.0), ("l1_cut", 0.55), ("l1_smooth", 0.15),
            ("flt_cut", 200.0), ("flt_res", 0.35),
            ("da_drv", 0.3),
            ("amp_rel", 80.0),
        ],
    },
    // ---------------------------------------------------------------- modern dnb / dubstep
    Preset {
        name: "Neuro 174",
        category: "DnB",
        values: &[
            ("gain", 1.04),
            ("o1_uni", 5.0), ("o1_det", 0.4), ("o2_wave", 0.0), ("o2_fine", 15.0), ("o2_lvl", 0.5),
            ("flt_kind", 2.0), ("flt_cut", 500.0), ("flt_res", 0.5), ("flt_drv", 0.3),
            ("l1_shape", 1.0), ("l1_sync", 0.0), ("l1_hz", 5.8), ("l1_trig", 2.0), ("l1_cut", 0.5),
            ("l2_hz", 0.7), ("l2_trig", 2.0), ("l2_fmt", 0.7),
            ("fmt_mix", 0.6), ("fmt_morph", 0.4), ("fmt_q", 0.6),
            ("da_kind", 2.0), ("da_drv", 0.6),
            ("db_kind", 3.0), ("db_drv", 0.25), ("db_mix", 0.5),
            ("tone_ott", 0.6),
        ],
    },
    Preset {
        name: "Jump Up Donk",
        category: "DnB",
        values: &[
            ("gain", 0.53),
            ("o1_wave", 1.0), ("o1_uni", 1.0), ("o2_lvl", 0.3),
            ("l1_shape", 3.0), ("l1_div", 7.0), ("l1_cut", 0.9),
            ("flt_cut", 100.0), ("flt_res", 0.65),
            ("da_kind", 2.0), ("da_drv", 0.5),
            ("amp_rel", 60.0),
            ("tone_ott", 0.4),
        ],
    },
    Preset {
        name: "Halftime Stepper",
        category: "DnB",
        values: &[
            ("gain", 0.81),
            ("pat_on", 1.0), ("pat_len", 8.0), ("pat_step", 1.0),
            ("pat_srate_1", 4.0), ("pat_srate_2", 4.0), ("pat_srate_3", 8.0), ("pat_srate_4", 10.0),
            ("pat_srate_5", 4.0), ("pat_srate_6", 7.0), ("pat_srate_7", 11.0), ("pat_srate_8", 12.0),
            ("pat_son_2", 0.0),
            ("pat_ssemi_7", 12.0),
            ("o1_uni", 5.0), ("o1_det", 0.35),
            ("o2_wave", 2.0), ("o2_semi", 12.0), ("o2_lvl", 0.0), ("o2_fm", 0.35),
            ("l1_cut", 0.7), ("l1_fm", 0.3),
            ("flt_res", 0.5),
            ("da_kind", 2.0), ("da_drv", 0.55),
            ("tone_ott", 0.5),
        ],
    },
    Preset {
        name: "Riddim Chop",
        category: "Dubstep",
        values: &[
            ("gain", 0.48),
            ("o1_wave", 1.0), ("o1_uni", 3.0), ("o1_det", 0.2),
            ("o2_wave", 2.0), ("o2_semi", 12.0), ("o2_lvl", 0.0), ("o2_fm", 0.3),
            ("l1_shape", 4.0), ("l1_div", 8.0), ("l1_cut", 0.7), ("l1_amp", 0.5), ("l1_smooth", 0.2),
            ("flt_cut", 160.0), ("flt_res", 0.5),
            ("da_kind", 2.0), ("da_drv", 0.6),
            ("tone_ott", 0.5),
            ("amp_rel", 70.0),
        ],
    },
    Preset {
        name: "Metal Grinder",
        category: "Dubstep",
        values: &[
            ("gain", 0.78),
            ("o1_uni", 3.0), ("o1_det", 0.25),
            ("o2_wave", 2.0), ("o2_semi", 7.0), ("o2_lvl", 0.0), ("o2_fm", 0.7),
            ("l1_div", 4.0), ("l1_cut", 0.6),
            ("l2_shape", 2.0), ("l2_sync", 1.0), ("l2_div", 10.0), ("l2_trig", 0.0), ("l2_fm", 0.5),
            ("flt_cut", 300.0), ("flt_res", 0.45),
            ("da_kind", 3.0), ("da_drv", 0.65),
            ("db_kind", 6.0), ("db_drv", 0.2),
            ("tone_ott", 0.7),
        ],
    },
];

/// Reset every parameter to its default, then apply `preset` on top (if any).
pub fn apply(params: &WubParams, setter: &ParamSetter, preset: Option<&Preset>) {
    for (id, ptr, _group) in params.param_map() {
        // SAFETY: the pointers come from `params`, which outlives this call.
        let normalized = unsafe {
            match preset.and_then(|p| p.values.iter().rev().find(|(k, _)| *k == id)) {
                Some((_, plain)) => ptr.preview_normalized(*plain),
                None => ptr.default_normalized_value(),
            }
        };
        unsafe {
            setter.raw_context.raw_begin_set_parameter(ptr);
            setter.raw_context.raw_set_parameter_normalized(ptr, normalized);
            setter.raw_context.raw_end_set_parameter(ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{Engine, Snap, Transport, MAXB};
    use std::collections::HashSet;

    #[test]
    fn every_preset_only_uses_real_ids_and_in_range_values() {
        let params = WubParams::default();
        let map = params.param_map();
        let ids: HashSet<&str> = map.iter().map(|(id, _, _)| id.as_str()).collect();
        for preset in PRESETS {
            let mut seen = HashSet::new();
            for (id, plain) in preset.values {
                assert!(ids.contains(id), "preset '{}' uses unknown parameter '{id}'", preset.name);
                assert!(seen.insert(*id), "preset '{}' sets '{id}' twice", preset.name);
                let ptr = map.iter().find(|(k, _, _)| k == id).unwrap().1;
                // Round-trip through the normalised range: out-of-range values come back clamped.
                let back = unsafe { ptr.preview_plain(ptr.preview_normalized(*plain)) };
                assert!(
                    (back - plain).abs() <= 0.01 * plain.abs().max(1.0),
                    "preset '{}': {id} = {plain} is out of range (clamps to {back})",
                    preset.name
                );
            }
        }
    }

    /// Apply a preset straight to an engine snapshot. Tests have no host to set parameters
    /// through, so this mirrors `Snap::read`.
    fn snap_for(preset: &Preset) -> Snap {
        use crate::params::*;
        use nih_plug::prelude::Enum;
        let mut s = Snap::read(&WubParams::default());
        for &(id, v) in preset.values {
            let b = v >= 0.5;
            let u = v as usize;
            let (head, tail) = id.split_once('_').unwrap_or((id, ""));
            match (head, tail) {
                ("gain", _) => {}
                ("vel", _) => s.vel_sens = v,
                ("glide", _) => s.glide_ms = v,
                ("legato", _) => s.legato = b,
                ("bend", _) => s.bend_range = v,
                ("wheel", _) => s.wheel_amt = v,
                ("noise", _) => s.noise = v,
                ("o1", "wave") => s.wave1 = Wave1::from_index(u),
                ("o1", "pwm") => s.pwm = v,
                ("o1", "uni") => s.voices = u,
                ("o1", "det") => s.detune = v,
                ("o1", "spr") => s.spread = v,
                ("o1", "lvl") => s.level1 = v,
                ("o1", "rnd") => s.rand = v,
                ("o2", "wave") => s.wave2 = Wave2::from_index(u),
                ("o2", "semi") => s.semi2 = v,
                ("o2", "fine") => s.fine2 = v,
                ("o2", "lvl") => s.level2 = v,
                ("o2", "fm") => s.fm = v,
                ("sub", "wave") => s.sub_wave = SubWave::from_index(u),
                ("sub", "oct") => s.sub_oct = v as i32,
                ("sub", "lvl") => s.sub_level = v,
                ("sub", "flt") => s.sub_to_filter = b,
                ("flt", "kind") => s.filt_kind = FilterKind::from_index(u),
                ("flt", "cut") => s.cutoff = v,
                ("flt", "res") => s.res = v,
                ("flt", "drv") => s.filt_drive = v,
                ("flt", "key") => s.keytrack = v,
                ("flt", "env") => s.env_amt = v,
                ("fmt", "mix") => s.fmt_mix = v,
                ("fmt", "morph") => s.fmt_morph = v,
                ("fmt", "q") => s.fmt_q = v,
                ("da" | "db", _) => {
                    let d = if head == "da" { &mut s.dist_a } else { &mut s.dist_b };
                    match tail {
                        "kind" => d.kind = DistKind::from_index(u),
                        "drv" => d.drive = v,
                        "mix" => d.mix = v,
                        _ => panic!("unmapped id {id}"),
                    }
                }
                ("tone", "lo") => s.low_cut = v,
                ("tone", "hi") => s.high_cut = v,
                ("tone", "ott") => s.ott = v,
                ("amp" | "fenv", _) => {
                    let e = if head == "amp" { &mut s.amp_env } else { &mut s.filt_env };
                    match tail {
                        "atk" => e.a = v,
                        "dec" => e.d = v,
                        "sus" => e.s = v,
                        "rel" => e.r = v,
                        _ => panic!("unmapped id {id}"),
                    }
                }
                ("l1" | "l2", _) => {
                    let l = &mut s.lfo[if head == "l1" { 0 } else { 1 }];
                    match tail {
                        "shape" => l.shape = LfoShape::from_index(u),
                        "sync" => l.sync = b,
                        "div" => l.beats = RateDiv::from_index(u).beats(),
                        "hz" => l.hz = v,
                        "trig" => l.trig = LfoTrig::from_index(u),
                        "phase" => l.phase = v,
                        "smooth" => l.smooth = v,
                        "bip" => l.bipolar = b,
                        "cut" => l.cut = v,
                        "res" => l.res = v,
                        "drv" => l.drive = v,
                        "fmt" => l.formant = v,
                        "fm" => l.fm = v,
                        "pwm" => l.pwm = v,
                        "pit" => l.pitch = v,
                        "amp" => l.amp = v,
                        _ => panic!("unmapped id {id}"),
                    }
                }
                ("pat", "on") => s.pat_on = b,
                ("pat", "len") => s.pat_len = u,
                ("pat", "step") => s.step_beats = StepLen::from_index(u).beats(),
                ("pat", _) => {
                    let (field, idx) = tail.split_once('_').expect("step id");
                    let st = &mut s.steps[idx.parse::<usize>().expect("step number") - 1];
                    match field {
                        "son" => st.on = b,
                        "srate" => st.beats = RateDiv::from_index(u).beats(),
                        "sdep" => st.depth = v,
                        "ssemi" => st.semi = v,
                        _ => panic!("unmapped id {id}"),
                    }
                }
                _ => panic!("unmapped id {id}"),
            }
        }
        s
    }

    /// Every preset must make a sound at a sensible level and stay finite.
    #[test]
    fn every_preset_sounds_at_a_sensible_level() {
        let mut problems = Vec::new();
        for preset in PRESETS {
            let snap = snap_for(preset);
            let mut e = Engine::new(48_000.0);
            e.note_on(36, 0.9, &snap);
            let mut x = Vec::new();
            for _ in 0..(2 * 48_000 / MAXB) {
                let mut l = [0.0f32; MAXB];
                let mut r = [0.0f32; MAXB];
                e.render(&snap, &Transport::default(), &mut l, &mut r);
                x.extend_from_slice(&l);
            }
            assert!(x.iter().all(|v| v.is_finite()), "preset '{}' is not finite", preset.name);
            let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            let rms = (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
            let gain = preset.values.iter().find(|(k, _)| *k == "gain").map(|(_, v)| *v).unwrap_or(util::db_to_gain(-3.0));
            let (peak, rms) = (peak * gain, rms * gain);
            println!("{:<22} peak {:>5.2}  rms {:>5.3}  (output {:.2})", preset.name, peak, rms, gain);
            // After the output gain every preset should land in the same loudness window.
            if !(0.45..=1.0).contains(&peak) {
                problems.push(format!("'{}' peaks at {peak:.2} (want 0.45..1.0)", preset.name));
            }
            if rms < 0.08 {
                problems.push(format!("'{}' has little energy (rms {rms:.3})", preset.name));
            }
        }
        assert!(problems.is_empty(), "preset problems:\n{}", problems.join("\n"));
    }

    #[test]
    fn preset_names_are_unique_and_the_first_is_the_default() {
        let names: HashSet<&str> = PRESETS.iter().map(|p| p.name).collect();
        assert_eq!(names.len(), PRESETS.len());
        assert!(PRESETS[0].values.is_empty());
        assert!(PRESETS.len() >= 20);
    }
}
