//! Factory presets.
//!
//! A preset lists only what differs from the default patch. Loading one resets *every* parameter
//! to its default and then applies the overrides, so nothing leaks over from the previous sound.
//!
//! Plain values: enums use their variant index, booleans 0 / 1, everything else its natural unit.
//!
//! - Wavetable (`*_tbl`): 0 Basic Shapes (0 sine, .25 tri, .5 saw, .75 square, 1 pulse), 1 PWM Sweep,
//!   2 Saw Resonance, 3 Vowels, 4 FM Bell, 5 Organ, 6 Spectral Chaos, 7 Metallic, 8 Sync Sweep,
//!   9 Growl, 10 Glass, 11 E-Piano, 12 Harmonic Series
//! - Warp (`*_w1m`): 0 off, 1 sync, 2 bend+, 3 bend-, 4 asym, 5 mirror, 6 twist, 7 FM, 8 fold,
//!   9 drive, 10 crush
//! - Filter (`flt_kind`): 0 LP12, 1 LP24, 2 HP12, 3 BP12, 4 notch, 5 peak, 6 comb+, 7 comb-
//! - LFO shape (`shp_n`): 0 sine, 1 tri, 2 saw up, 3 saw down, 4 square, 5 S&H, 6 smooth random
//! - LFO division (`div_n`): 0 1/16, 1 1/8, 2 1/4, 3 1/2, 4 bar, 5 2 bars, 6 4 bars
//! - Voice mode (`vmode`): 0 poly, 1 mono, 2 legato. Distortion (`dst_kind`): 0 soft, 1 hard, 2 fold, 3 crush
//! - Mod source (`src_n`): 1-4 LFO 1-4, 5 Env 2, 6 Env 3, 7 velocity, 8 note, 9 mod wheel,
//!   10 aftertouch, 11-14 macro 1-4
//! - Mod destination (`dst_n`): 1 A pos, 2 B pos, 3 A pitch, 4 B pitch, 5 A level, 6 B level, 7 A pan,
//!   8 B pan, 9 A warp 1, 10 A warp 2, 11 B warp 1, 12 B warp 2, 13 A detune, 14 B detune, 15 sub,
//!   16 noise, 17 cutoff, 18 resonance, 19 filter drive, 20 amp

use crate::params::SzParams;
use nih_plug::prelude::*;

pub struct Preset {
    pub name: &'static str,
    pub category: &'static str,
    pub values: &'static [(&'static str, f32)],
}

pub const PRESETS: &[Preset] = &[
    Preset { name: "Init Saw", category: "Init", values: &[] },
    // ------------------------------------------------------------------------------------ bass
    Preset {
        name: "Sub Zero",
        category: "Bass",
        values: &[
            ("gain", 1.50),
            ("vmode", 1.0), ("glide", 35.0),
            ("a_pos", 0.0), ("a_rnd", 0.0), ("a_lvl", 0.9),
            ("sub_on", 1.0), ("sub_lvl", 0.35),
            ("flt_cut", 320.0), ("flt_res", 0.1), ("flt_env", 0.3), ("flt_drv", 0.25),
            ("atk_1", 2.0), ("dec_1", 700.0), ("sus_1", 0.7), ("rel_1", 220.0),
            ("dec_2", 260.0),
        ],
    },
    Preset {
        name: "Reese Engine",
        category: "Bass",
        values: &[
            ("gain", 1.20),
            ("vmode", 2.0), ("glide", 60.0),
            ("a_pos", 0.5), ("a_uni", 3.0), ("a_det", 0.34), ("a_spr", 0.5), ("a_lvl", 0.7),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.65), ("b_uni", 3.0), ("b_det", 0.42),
            ("b_fine", -9.0), ("b_spr", 0.5),
            ("sub_on", 1.0), ("sub_lvl", 0.5),
            ("flt_cut", 850.0), ("flt_res", 0.22), ("flt_drv", 0.35),
            ("src_1", 2.0), ("dst_1", 17.0), ("amt_1", 0.14), ("rate_2", 0.13), ("ret_2", 0.0),
            ("sus_1", 1.0), ("rel_1", 180.0),
            ("cho_on", 1.0), ("cho_mix", 0.25),
        ],
    },
    Preset {
        name: "Acid Line",
        category: "Bass",
        values: &[
            ("gain", 0.65),
            ("vmode", 2.0), ("glide", 55.0),
            ("a_pos", 0.5), ("a_rnd", 0.0), ("a_lvl", 0.85),
            ("flt_cut", 210.0), ("flt_res", 0.8), ("flt_drv", 0.4), ("flt_env", 0.5), ("flt_key", 0.3),
            ("dec_2", 230.0),
            ("src_2", 7.0), ("dst_2", 17.0), ("amt_2", 0.3),
            ("dec_1", 500.0), ("sus_1", 0.6), ("rel_1", 90.0),
            ("dst_on", 1.0), ("dst_drv", 0.4), ("dst_mix", 0.7),
            ("dly_on", 1.0), ("dly_time", 375.0), ("dly_fb", 0.35), ("dly_mix", 0.16),
        ],
    },
    Preset {
        name: "Growl Bass",
        category: "Bass",
        values: &[
            ("gain", 0.55),
            ("vmode", 2.0), ("glide", 25.0),
            ("a_tbl", 9.0), ("a_pos", 0.15), ("a_rnd", 0.0), ("a_lvl", 0.85),
            ("a_w1m", 9.0), ("a_w1a", 0.45),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.3),
            ("shp_1", 1.0), ("sync_1", 1.0), ("div_1", 1.0),
            ("src_2", 1.0), ("dst_2", 17.0), ("amt_2", 0.18),
            ("src_3", 9.0), ("dst_3", 1.0), ("amt_3", 0.4),
            ("sub_on", 1.0), ("sub_lvl", 0.6),
            ("flt_cut", 2200.0), ("flt_res", 0.3), ("flt_drv", 0.45),
            ("sus_1", 1.0), ("rel_1", 120.0),
            ("dst_on", 1.0), ("dst_drv", 0.4), ("dst_mix", 0.8),
        ],
    },
    Preset {
        name: "FM Wobble",
        category: "Bass",
        values: &[
            ("gain", 0.75),
            ("vmode", 2.0), ("glide", 30.0),
            ("a_pos", 0.0), ("a_rnd", 0.0), ("a_lvl", 0.9),
            ("a_w1m", 7.0), ("a_w1a", 0.3),
            ("src_1", 1.0), ("dst_1", 9.0), ("amt_1", 0.3),
            ("shp_1", 1.0), ("sync_1", 1.0), ("div_1", 2.0),
            ("sub_on", 1.0), ("sub_lvl", 0.55),
            ("flt_cut", 3500.0), ("flt_res", 0.2),
            ("sus_1", 1.0), ("rel_1", 140.0),
            ("dst_on", 1.0), ("dst_kind", 0.0), ("dst_drv", 0.3), ("dst_mix", 0.6),
        ],
    },
    Preset {
        name: "House Pluck Bass",
        category: "Bass",
        values: &[
            ("gain", 1.65),
            ("a_pos", 0.75), ("a_rnd", 0.0), ("a_lvl", 0.7),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.5), ("b_rnd", 0.0),
            ("sub_on", 1.0), ("sub_lvl", 0.45),
            ("flt_cut", 150.0), ("flt_res", 0.3), ("flt_env", 0.62), ("flt_drv", 0.2),
            ("dec_2", 190.0),
            ("dec_1", 380.0), ("sus_1", 0.0), ("rel_1", 140.0),
        ],
    },
    // ------------------------------------------------------------------------------------ lead
    Preset {
        name: "Supersaw Anthem",
        category: "Lead",
        values: &[
            ("gain", 0.56),
            ("a_pos", 0.5), ("a_uni", 7.0), ("a_det", 0.32), ("a_bln", 0.85), ("a_spr", 0.9), ("a_lvl", 0.7),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.55), ("b_uni", 7.0), ("b_det", 0.46),
            ("b_semi", 12.0), ("b_spr", 1.0),
            ("flt_kind", 0.0), ("flt_cut", 7500.0), ("flt_res", 0.1), ("flt_env", 0.2),
            ("dec_2", 600.0), ("sus_2", 0.3),
            ("atk_1", 3.0), ("sus_1", 0.9), ("rel_1", 380.0),
            ("dly_on", 1.0), ("dly_time", 345.0), ("dly_fb", 0.35), ("dly_mix", 0.18),
            ("rev_on", 1.0), ("rev_size", 0.65), ("rev_mix", 0.25),
        ],
    },
    Preset {
        name: "Sync Screamer",
        category: "Lead",
        values: &[
            ("gain", 1.80),
            ("vmode", 2.0), ("glide", 70.0),
            ("a_tbl", 8.0), ("a_pos", 0.2), ("a_uni", 2.0), ("a_det", 0.12), ("a_lvl", 1.0),
            ("src_1", 5.0), ("dst_1", 1.0), ("amt_1", 0.5),
            ("dec_2", 700.0), ("sus_2", 0.25),
            ("src_2", 9.0), ("dst_2", 1.0), ("amt_2", 0.35),
            ("src_3", 2.0), ("dst_3", 3.0), ("amt_3", 0.004), ("shp_2", 0.0), ("rate_2", 5.6),
            ("flt_cut", 11000.0), ("flt_res", 0.1),
            ("sus_1", 0.9), ("rel_1", 200.0),
            ("dly_on", 1.0), ("dly_time", 320.0), ("dly_mix", 0.22),
            ("rev_on", 1.0), ("rev_mix", 0.15),
        ],
    },
    Preset {
        name: "Pulse Width Lead",
        category: "Lead",
        values: &[
            ("gain", 1.80),
            ("vmode", 2.0), ("glide", 40.0),
            ("a_tbl", 1.0), ("a_pos", 0.3), ("a_lvl", 1.0), ("a_rnd", 0.0),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.25), ("rate_1", 0.7), ("ret_1", 0.0),
            ("src_2", 2.0), ("dst_2", 3.0), ("amt_2", 0.004), ("shp_2", 0.0), ("rate_2", 5.4),
            ("flt_kind", 0.0), ("flt_cut", 5200.0), ("flt_res", 0.2),
            ("sus_1", 0.9), ("rel_1", 160.0),
            ("dly_on", 1.0), ("dly_time", 300.0), ("dly_mix", 0.2),
        ],
    },
    Preset {
        name: "Talkbox Lead",
        category: "Lead",
        values: &[
            ("gain", 1.20),
            ("vmode", 2.0), ("glide", 50.0),
            ("a_tbl", 3.0), ("a_pos", 0.5), ("a_uni", 3.0), ("a_det", 0.14), ("a_lvl", 0.85),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.45), ("shp_1", 1.0), ("rate_1", 0.6), ("ret_1", 0.0),
            ("src_2", 9.0), ("dst_2", 1.0), ("amt_2", 0.4),
            ("flt_kind", 0.0), ("flt_cut", 6500.0),
            ("sus_1", 0.9), ("rel_1", 180.0),
            ("rev_on", 1.0), ("rev_mix", 0.18),
        ],
    },
    Preset {
        name: "Hoover",
        category: "Lead",
        values: &[
            ("gain", 0.80),
            ("vmode", 2.0), ("glide", 120.0),
            ("a_tbl", 1.0), ("a_pos", 0.35), ("a_uni", 7.0), ("a_det", 0.55), ("a_lvl", 0.7),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.6), ("b_uni", 5.0), ("b_det", 0.5), ("b_semi", -12.0),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.25), ("rate_1", 3.2), ("ret_1", 0.0),
            ("src_2", 6.0), ("dst_2", 3.0), ("amt_2", -0.1),
            ("src_3", 6.0), ("dst_3", 4.0), ("amt_3", -0.1),
            ("dec_3", 500.0),
            ("flt_kind", 0.0), ("flt_cut", 7000.0),
            ("sus_1", 1.0), ("rel_1", 250.0),
            ("cho_on", 1.0), ("cho_dep", 0.8), ("cho_mix", 0.6),
            ("rev_on", 1.0), ("rev_mix", 0.2),
        ],
    },
    Preset {
        name: "Chip Octaves",
        category: "Lead",
        values: &[
            ("gain", 0.90),
            ("a_tbl", 0.0), ("a_pos", 0.75), ("a_rnd", 0.0), ("a_semi", 6.0), ("a_lvl", 0.7),
            ("src_1", 4.0), ("dst_1", 3.0), ("amt_1", 0.25),
            ("shp_4", 4.0), ("sync_4", 1.0), ("div_4", 0.0),
            ("flt_kind", 0.0), ("flt_cut", 9000.0),
            ("sus_1", 0.7), ("rel_1", 90.0),
            ("dly_on", 1.0), ("dly_time", 250.0), ("dly_mix", 0.2),
        ],
    },
    // ------------------------------------------------------------------------------------ pluck
    Preset {
        name: "Trance Pluck",
        category: "Pluck",
        values: &[
            ("gain", 1.10),
            ("a_pos", 0.5), ("a_uni", 5.0), ("a_det", 0.28), ("a_lvl", 0.7),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.45), ("b_uni", 5.0), ("b_det", 0.35), ("b_semi", 12.0),
            ("flt_cut", 280.0), ("flt_res", 0.25), ("flt_env", 0.7),
            ("dec_2", 280.0), ("sus_2", 0.05), ("rel_2", 280.0),
            ("dec_1", 520.0), ("sus_1", 0.15), ("rel_1", 320.0),
            ("dly_on", 1.0), ("dly_time", 375.0), ("dly_fb", 0.45), ("dly_mix", 0.28),
            ("rev_on", 1.0), ("rev_size", 0.6), ("rev_mix", 0.25),
        ],
    },
    Preset {
        name: "Digital Pluck",
        category: "Pluck",
        values: &[
            ("gain", 1.00),
            ("a_tbl", 12.0), ("a_pos", 0.12), ("a_uni", 2.0), ("a_det", 0.1), ("a_lvl", 0.8),
            ("src_1", 5.0), ("dst_1", 1.0), ("amt_1", 0.6),
            ("dec_2", 240.0),
            ("flt_kind", 0.0), ("flt_cut", 12000.0),
            ("dec_1", 700.0), ("sus_1", 0.0), ("rel_1", 500.0),
            ("dly_on", 1.0), ("dly_time", 330.0), ("dly_fb", 0.4), ("dly_mix", 0.28),
            ("rev_on", 1.0), ("rev_mix", 0.28),
        ],
    },
    Preset {
        name: "Plucked String",
        category: "Pluck",
        values: &[
            ("gain", 1.20),
            ("a_on", 0.0),
            ("noi_on", 1.0), ("noi_lvl", 0.0), ("noi_flt", 1.0),
            ("src_1", 5.0), ("dst_1", 16.0), ("amt_1", 1.0),
            ("dec_2", 45.0), ("rel_2", 45.0),
            ("flt_kind", 6.0), ("flt_cut", 261.6), ("flt_key", 1.0), ("flt_res", 0.96), ("flt_drv", 0.6),
            ("dec_1", 100.0), ("sus_1", 1.0), ("rel_1", 900.0),
            ("rev_on", 1.0), ("rev_size", 0.5), ("rev_mix", 0.2),
        ],
    },
    Preset {
        name: "Kalimba",
        category: "Pluck",
        values: &[
            ("gain", 0.76),
            ("a_tbl", 10.0), ("a_pos", 0.05), ("a_rnd", 0.0), ("a_lvl", 0.9),
            ("src_1", 5.0), ("dst_1", 1.0), ("amt_1", 0.35),
            ("dec_2", 90.0),
            ("flt_kind", 0.0), ("flt_cut", 9000.0),
            ("dec_1", 650.0), ("sus_1", 0.0), ("rel_1", 650.0),
            ("rev_on", 1.0), ("rev_size", 0.55), ("rev_mix", 0.3),
        ],
    },
    // ------------------------------------------------------------------------------------ keys
    Preset {
        name: "Tine Piano",
        category: "Keys",
        values: &[
            ("gain", 0.70),
            ("a_tbl", 11.0), ("a_pos", 0.2), ("a_rnd", 0.0), ("a_lvl", 0.9),
            ("src_1", 7.0), ("dst_1", 1.0), ("amt_1", 0.45),
            ("src_2", 5.0), ("dst_2", 1.0), ("amt_2", 0.2),
            ("src_3", 1.0), ("dst_3", 7.0), ("amt_3", 0.3), ("rate_1", 4.2), ("ret_1", 0.0),
            ("dec_2", 320.0),
            ("flt_kind", 0.0), ("flt_cut", 7000.0), ("flt_key", 0.3),
            ("dec_1", 2400.0), ("sus_1", 0.12), ("rel_1", 420.0),
            ("cho_on", 1.0), ("cho_rate", 0.8), ("cho_dep", 0.3), ("cho_mix", 0.3),
            ("rev_on", 1.0), ("rev_mix", 0.18),
        ],
    },
    Preset {
        name: "FM Bell",
        category: "Keys",
        values: &[
            ("gain", 0.60),
            ("a_tbl", 4.0), ("a_pos", 0.12), ("a_rnd", 0.0), ("a_lvl", 0.8),
            ("src_1", 5.0), ("dst_1", 1.0), ("amt_1", 0.35),
            ("dec_2", 900.0),
            ("b_on", 1.0), ("b_tbl", 10.0), ("b_pos", 0.5), ("b_lvl", 0.3), ("b_semi", 12.0), ("b_rnd", 0.0),
            ("flt_kind", 0.0), ("flt_cut", 14000.0),
            ("dec_1", 2600.0), ("sus_1", 0.0), ("rel_1", 2600.0),
            ("dly_on", 1.0), ("dly_time", 420.0), ("dly_mix", 0.15),
            ("rev_on", 1.0), ("rev_size", 0.8), ("rev_mix", 0.4),
        ],
    },
    Preset {
        name: "Drawbar Organ",
        category: "Keys",
        values: &[
            ("gain", 0.60),
            ("a_tbl", 5.0), ("a_pos", 0.4), ("a_rnd", 0.0), ("a_lvl", 0.8),
            ("src_1", 9.0), ("dst_1", 1.0), ("amt_1", 0.5),
            ("b_on", 1.0), ("b_tbl", 5.0), ("b_pos", 0.9), ("b_lvl", 0.25), ("b_semi", 12.0), ("b_rnd", 0.0),
            ("flt_kind", 0.0), ("flt_cut", 9000.0),
            ("atk_1", 3.0), ("dec_1", 100.0), ("sus_1", 1.0), ("rel_1", 70.0),
            ("dst_on", 1.0), ("dst_drv", 0.22), ("dst_mix", 0.5),
            ("cho_on", 1.0), ("cho_rate", 5.2), ("cho_dep", 0.25), ("cho_mix", 0.5),
            ("rev_on", 1.0), ("rev_mix", 0.15),
        ],
    },
    // ------------------------------------------------------------------------------------ pad
    Preset {
        name: "Warm Analog Pad",
        category: "Pad",
        values: &[
            ("gain", 0.70),
            ("a_pos", 0.5), ("a_uni", 5.0), ("a_det", 0.2), ("a_lvl", 0.65),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.35), ("b_lvl", 0.55), ("b_uni", 5.0), ("b_det", 0.3), ("b_semi", -12.0),
            ("flt_cut", 1300.0), ("flt_res", 0.1), ("flt_key", 0.3),
            ("src_1", 1.0), ("dst_1", 17.0), ("amt_1", 0.1), ("rate_1", 0.12), ("ret_1", 0.0),
            ("atk_1", 650.0), ("dec_1", 1500.0), ("sus_1", 0.8), ("rel_1", 1600.0),
            ("cho_on", 1.0), ("cho_mix", 0.5),
            ("rev_on", 1.0), ("rev_size", 0.8), ("rev_mix", 0.4),
        ],
    },
    Preset {
        name: "Glass Cathedral",
        category: "Pad",
        values: &[
            ("gain", 0.60),
            ("a_tbl", 10.0), ("a_pos", 0.25), ("a_uni", 4.0), ("a_det", 0.15), ("a_lvl", 0.7),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.3), ("shp_1", 1.0), ("rate_1", 0.08), ("ret_1", 0.0),
            ("b_on", 1.0), ("b_tbl", 12.0), ("b_pos", 0.15), ("b_lvl", 0.3), ("b_semi", 12.0), ("b_uni", 3.0), ("b_det", 0.2),
            ("src_2", 2.0), ("dst_2", 2.0), ("amt_2", 0.2), ("rate_2", 0.05), ("ret_2", 0.0),
            ("flt_kind", 0.0), ("flt_cut", 6000.0),
            ("atk_1", 900.0), ("sus_1", 0.9), ("rel_1", 2600.0),
            ("cho_on", 1.0), ("cho_mix", 0.4),
            ("rev_on", 1.0), ("rev_size", 0.92), ("rev_mix", 0.5),
        ],
    },
    Preset {
        name: "Vowel Choir",
        category: "Pad",
        values: &[
            ("gain", 0.90),
            ("a_tbl", 3.0), ("a_pos", 0.15), ("a_uni", 6.0), ("a_det", 0.22), ("a_lvl", 0.7),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.18), ("rate_1", 0.1), ("ret_1", 0.0),
            ("b_on", 1.0), ("b_tbl", 3.0), ("b_pos", 0.6), ("b_lvl", 0.5), ("b_semi", 12.0), ("b_uni", 4.0), ("b_det", 0.2),
            ("noi_on", 1.0), ("noi_lvl", 0.04),
            ("flt_kind", 0.0), ("flt_cut", 4500.0),
            ("atk_1", 500.0), ("sus_1", 0.9), ("rel_1", 1300.0),
            ("cho_on", 1.0), ("cho_mix", 0.45),
            ("rev_on", 1.0), ("rev_size", 0.85), ("rev_mix", 0.45),
        ],
    },
    Preset {
        name: "String Machine",
        category: "Pad",
        values: &[
            ("gain", 0.60),
            ("a_tbl", 1.0), ("a_pos", 0.2), ("a_uni", 4.0), ("a_det", 0.18), ("a_lvl", 0.65),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.2), ("rate_1", 0.35), ("ret_1", 0.0),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.45), ("b_uni", 4.0), ("b_det", 0.22), ("b_semi", 12.0),
            ("flt_kind", 0.0), ("flt_cut", 5200.0),
            ("atk_1", 250.0), ("sus_1", 0.9), ("rel_1", 750.0),
            ("cho_on", 1.0), ("cho_rate", 0.9), ("cho_dep", 0.7), ("cho_mix", 0.6),
            ("rev_on", 1.0), ("rev_mix", 0.25),
        ],
    },
    Preset {
        name: "Dark Matter",
        category: "Pad",
        values: &[
            ("gain", 1.10),
            ("a_tbl", 6.0), ("a_pos", 0.3), ("a_uni", 3.0), ("a_det", 0.2), ("a_lvl", 0.7),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.3), ("shp_1", 6.0), ("rate_1", 0.09), ("ret_1", 0.0),
            ("b_on", 1.0), ("b_tbl", 7.0), ("b_pos", 0.3), ("b_lvl", 0.35), ("b_semi", -12.0),
            ("src_2", 2.0), ("dst_2", 2.0), ("amt_2", 0.3), ("rate_2", 0.05), ("ret_2", 0.0),
            ("src_3", 2.0), ("dst_3", 17.0), ("amt_3", 0.2),
            ("flt_cut", 900.0), ("flt_res", 0.35),
            ("atk_1", 1500.0), ("sus_1", 0.9), ("rel_1", 3000.0),
            ("dly_on", 1.0), ("dly_time", 520.0), ("dly_fb", 0.5), ("dly_mix", 0.2),
            ("rev_on", 1.0), ("rev_size", 0.95), ("rev_mix", 0.55),
        ],
    },
    // ------------------------------------------------------------------------------------ sequence
    Preset {
        name: "Gated Stepper",
        category: "Sequence",
        values: &[
            ("gain", 0.40),
            ("a_pos", 0.5), ("a_uni", 3.0), ("a_det", 0.2), ("a_lvl", 0.75),
            ("src_1", 3.0), ("dst_1", 17.0), ("amt_1", 0.4), ("sync_3", 1.0), ("div_3", 0.0), ("ret_3", 0.0),
            ("src_2", 4.0), ("dst_2", 20.0), ("amt_2", 0.6), ("sync_4", 1.0), ("div_4", 1.0), ("ret_4", 0.0),
            ("flt_cut", 600.0), ("flt_res", 0.5),
            ("sus_1", 1.0), ("rel_1", 200.0),
            ("dly_on", 1.0), ("dly_time", 375.0), ("dly_fb", 0.4), ("dly_mix", 0.22),
            ("rev_on", 1.0), ("rev_mix", 0.18),
        ],
    },
    // ------------------------------------------------------------------------------------ synthwave / 80s
    Preset {
        name: "Neon Bass",
        category: "Synthwave",
        values: &[
            ("gain", 1.65),
            ("vmode", 1.0),
            ("a_pos", 0.5), ("a_rnd", 0.0), ("a_lvl", 0.8),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.75), ("b_lvl", 0.5), ("b_fine", 6.0), ("b_rnd", 0.0),
            ("flt_cut", 360.0), ("flt_res", 0.3), ("flt_env", 0.45), ("flt_drv", 0.2),
            ("dec_2", 180.0), ("sus_2", 0.15),
            ("dec_1", 320.0), ("sus_1", 0.6), ("rel_1", 110.0),
            ("cho_on", 1.0), ("cho_mix", 0.2),
        ],
    },
    Preset {
        name: "FM Night Bass",
        category: "Synthwave",
        values: &[
            ("gain", 1.73),
            ("vmode", 1.0),
            ("a_tbl", 11.0), ("a_pos", 0.45), ("a_rnd", 0.0), ("a_lvl", 0.9),
            ("src_1", 5.0), ("dst_1", 1.0), ("amt_1", 0.35),
            ("dec_2", 150.0),
            ("sub_on", 1.0), ("sub_lvl", 0.4),
            ("flt_kind", 0.0), ("flt_cut", 3200.0),
            ("dec_1", 500.0), ("sus_1", 0.35), ("rel_1", 120.0),
        ],
    },
    Preset {
        name: "Outrun Lead",
        category: "Synthwave",
        values: &[
            ("gain", 0.91),
            ("vmode", 2.0), ("glide", 60.0),
            ("a_pos", 0.5), ("a_uni", 3.0), ("a_det", 0.18), ("a_lvl", 0.85),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.75), ("b_lvl", 0.4), ("b_semi", 12.0),
            ("src_2", 2.0), ("dst_2", 3.0), ("amt_2", 0.004), ("shp_2", 0.0), ("rate_2", 5.5),
            ("flt_kind", 0.0), ("flt_cut", 4500.0), ("flt_res", 0.25), ("flt_env", 0.25),
            ("dec_2", 500.0), ("sus_2", 0.5),
            ("sus_1", 0.9), ("rel_1", 220.0),
            ("cho_on", 1.0), ("cho_mix", 0.3),
            ("dly_on", 1.0), ("dly_time", 300.0), ("dly_fb", 0.4), ("dly_mix", 0.25),
            ("rev_on", 1.0), ("rev_mix", 0.25),
        ],
    },
    Preset {
        name: "Juno Pad",
        category: "Synthwave",
        values: &[
            ("gain", 0.68),
            ("a_tbl", 1.0), ("a_pos", 0.25), ("a_uni", 2.0), ("a_det", 0.1), ("a_lvl", 0.75),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.15), ("rate_1", 0.4), ("ret_1", 0.0),
            ("sub_on", 1.0), ("sub_shp", 3.0), ("sub_lvl", 0.3), ("sub_flt", 1.0),
            ("flt_cut", 1700.0), ("flt_res", 0.15), ("flt_env", 0.15), ("flt_key", 0.3),
            ("atk_2", 400.0), ("dec_2", 1200.0), ("sus_2", 0.6),
            ("atk_1", 350.0), ("sus_1", 0.9), ("rel_1", 1200.0),
            ("cho_on", 1.0), ("cho_rate", 0.5), ("cho_dep", 0.7), ("cho_mix", 0.7),
            ("rev_on", 1.0), ("rev_size", 0.75), ("rev_mix", 0.35),
        ],
    },
    Preset {
        name: "Poly Brass 84",
        category: "Synthwave",
        values: &[
            ("gain", 0.81),
            ("a_pos", 0.5), ("a_uni", 2.0), ("a_det", 0.15), ("a_lvl", 0.75),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.65), ("b_fine", 8.0),
            ("flt_cut", 480.0), ("flt_res", 0.15), ("flt_env", 0.55), ("flt_key", 0.3),
            ("atk_2", 60.0), ("dec_2", 500.0), ("sus_2", 0.45), ("rel_2", 250.0),
            ("atk_1", 15.0), ("sus_1", 0.9), ("rel_1", 250.0),
            ("cho_on", 1.0), ("cho_mix", 0.25),
            ("rev_on", 1.0), ("rev_mix", 0.2),
        ],
    },
    Preset {
        name: "Analog Strings",
        category: "Synthwave",
        values: &[
            ("gain", 0.87),
            ("a_pos", 0.5), ("a_uni", 4.0), ("a_det", 0.22), ("a_lvl", 0.65),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.4), ("b_uni", 4.0), ("b_det", 0.26), ("b_semi", 12.0),
            ("flt_kind", 0.0), ("flt_cut", 3800.0), ("flt_key", 0.2),
            ("atk_1", 300.0), ("sus_1", 0.9), ("rel_1", 900.0),
            ("cho_on", 1.0), ("cho_dep", 0.6), ("cho_mix", 0.6),
            ("rev_on", 1.0), ("rev_size", 0.75), ("rev_mix", 0.35),
        ],
    },
    Preset {
        name: "Blade Brass",
        category: "Synthwave",
        values: &[
            ("gain", 0.77),
            ("a_pos", 0.5), ("a_uni", 2.0), ("a_det", 0.2), ("a_lvl", 0.75),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.6), ("b_fine", -7.0),
            ("flt_cut", 340.0), ("flt_res", 0.2), ("flt_env", 0.5), ("flt_key", 0.3),
            ("atk_2", 250.0), ("dec_2", 1500.0), ("sus_2", 0.6), ("rel_2", 1500.0),
            ("src_2", 10.0), ("dst_2", 17.0), ("amt_2", 0.3),
            ("src_3", 1.0), ("dst_3", 17.0), ("amt_3", 0.04), ("rate_1", 5.0), ("ret_1", 0.0),
            ("atk_1", 120.0), ("sus_1", 0.9), ("rel_1", 1800.0),
            ("cho_on", 1.0), ("cho_mix", 0.3),
            ("rev_on", 1.0), ("rev_size", 0.9), ("rev_mix", 0.45),
        ],
    },
    Preset {
        name: "Sunset Keys",
        category: "Synthwave",
        values: &[
            ("a_pos", 0.3), ("a_rnd", 0.0), ("a_lvl", 0.85),
            ("b_on", 1.0), ("b_tbl", 11.0), ("b_pos", 0.3), ("b_lvl", 0.4), ("b_rnd", 0.0),
            ("flt_cut", 1100.0), ("flt_env", 0.35), ("flt_key", 0.4),
            ("dec_2", 600.0), ("sus_2", 0.2),
            ("src_2", 7.0), ("dst_2", 17.0), ("amt_2", 0.2),
            ("dec_1", 1500.0), ("sus_1", 0.3), ("rel_1", 500.0),
            ("cho_on", 1.0), ("cho_mix", 0.5),
            ("rev_on", 1.0), ("rev_mix", 0.3),
        ],
    },
    Preset {
        name: "Digital Tines 86",
        category: "Synthwave",
        values: &[
            ("a_tbl", 11.0), ("a_pos", 0.5), ("a_rnd", 0.0), ("a_lvl", 0.85),
            ("src_1", 7.0), ("dst_1", 1.0), ("amt_1", 0.35),
            ("b_on", 1.0), ("b_tbl", 10.0), ("b_pos", 0.3), ("b_lvl", 0.3), ("b_semi", 12.0), ("b_rnd", 0.0),
            ("flt_kind", 0.0), ("flt_cut", 9000.0), ("flt_key", 0.3),
            ("dec_1", 1800.0), ("sus_1", 0.1), ("rel_1", 600.0),
            ("cho_on", 1.0), ("cho_mix", 0.4),
            ("rev_on", 1.0), ("rev_size", 0.7), ("rev_mix", 0.3),
        ],
    },
    Preset {
        name: "Arp Pluck",
        category: "Synthwave",
        values: &[
            ("gain", 0.77),
            ("a_pos", 0.5), ("a_rnd", 0.0), ("a_lvl", 0.8),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.75), ("b_lvl", 0.45), ("b_rnd", 0.0),
            ("flt_cut", 240.0), ("flt_res", 0.3), ("flt_env", 0.65),
            ("dec_2", 140.0), ("rel_2", 140.0),
            ("dec_1", 230.0), ("sus_1", 0.0), ("rel_1", 200.0),
            ("dly_on", 1.0), ("dly_time", 262.0), ("dly_fb", 0.45), ("dly_mix", 0.3),
            ("rev_on", 1.0), ("rev_mix", 0.2),
        ],
    },
    Preset {
        name: "Hyper Arp Lead",
        category: "Synthwave",
        values: &[
            ("gain", 0.68),
            ("a_pos", 0.5), ("a_uni", 5.0), ("a_det", 0.25), ("a_lvl", 0.75),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.75), ("b_lvl", 0.35), ("b_semi", 12.0),
            ("flt_kind", 0.0), ("flt_cut", 5500.0), ("flt_res", 0.15), ("flt_env", 0.3),
            ("dec_2", 120.0), ("sus_2", 0.2),
            ("dec_1", 180.0), ("sus_1", 0.35), ("rel_1", 90.0),
            ("dly_on", 1.0), ("dly_time", 262.0), ("dly_fb", 0.35), ("dly_mix", 0.22),
            ("rev_on", 1.0), ("rev_mix", 0.15),
        ],
    },
    Preset {
        name: "Laser Zap",
        category: "Synthwave",
        values: &[
            ("gain", 0.71),
            ("a_pos", 0.0), ("a_rnd", 0.0), ("a_lvl", 0.9),
            ("src_1", 6.0), ("dst_1", 3.0), ("amt_1", 0.6),
            ("dec_3", 140.0),
            ("flt_on", 0.0),
            ("dec_1", 260.0), ("sus_1", 0.0), ("rel_1", 200.0),
            ("dly_on", 1.0), ("dly_time", 180.0), ("dly_fb", 0.4), ("dly_mix", 0.25),
            ("rev_on", 1.0), ("rev_mix", 0.3),
        ],
    },
    // ------------------------------------------------------------------------------------ drum and bass / dubstep
    Preset {
        name: "Neuro Reese",
        category: "DnB",
        values: &[
            ("gain", 0.74),
            ("vmode", 2.0), ("glide", 45.0),
            ("a_pos", 0.5), ("a_uni", 3.0), ("a_det", 0.4), ("a_spr", 0.5), ("a_lvl", 0.7),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.65), ("b_uni", 3.0), ("b_det", 0.45), ("b_fine", -14.0), ("b_spr", 0.5),
            ("sub_on", 1.0), ("sub_lvl", 0.6),
            ("flt_kind", 4.0), ("flt_cut", 900.0), ("flt_res", 0.5), ("flt_drv", 0.3),
            ("src_1", 1.0), ("dst_1", 17.0), ("amt_1", 0.3), ("shp_1", 6.0), ("rate_1", 0.8), ("ret_1", 0.0),
            ("sus_1", 1.0), ("rel_1", 140.0),
            ("dst_on", 1.0), ("dst_kind", 1.0), ("dst_drv", 0.4), ("dst_mix", 0.7),
            ("cho_on", 1.0), ("cho_mix", 0.2),
        ],
    },
    Preset {
        name: "Wave Growl 174",
        category: "DnB",
        values: &[
            ("gain", 0.75),
            ("vmode", 2.0), ("glide", 20.0),
            ("a_tbl", 9.0), ("a_pos", 0.1), ("a_rnd", 0.0), ("a_lvl", 0.85),
            ("a_w1m", 8.0), ("a_w1a", 0.3),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.4), ("shp_1", 2.0), ("sync_1", 1.0), ("div_1", 1.0),
            ("src_2", 9.0), ("dst_2", 9.0), ("amt_2", 0.4),
            ("sub_on", 1.0), ("sub_lvl", 0.6),
            ("flt_cut", 3200.0), ("flt_res", 0.25), ("flt_drv", 0.35),
            ("sus_1", 1.0), ("rel_1", 100.0),
            ("dst_on", 1.0), ("dst_drv", 0.45), ("dst_mix", 0.8),
        ],
    },
    Preset {
        name: "Jump Up Wobble",
        category: "DnB",
        values: &[
            ("gain", 0.81),
            ("vmode", 2.0), ("glide", 20.0),
            ("a_tbl", 2.0), ("a_pos", 0.05), ("a_rnd", 0.0), ("a_lvl", 0.85),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.3), ("shp_1", 3.0), ("sync_1", 1.0), ("div_1", 2.0),
            ("sub_on", 1.0), ("sub_lvl", 0.6),
            ("flt_cut", 2600.0), ("flt_res", 0.2),
            ("sus_1", 1.0), ("rel_1", 90.0),
            ("dst_on", 1.0), ("dst_kind", 1.0), ("dst_drv", 0.3), ("dst_mix", 0.7),
        ],
    },
    Preset {
        name: "Foghorn",
        category: "DnB",
        values: &[
            ("gain", 0.74),
            ("vmode", 2.0), ("glide", 80.0),
            ("a_tbl", 1.0), ("a_pos", 0.4), ("a_uni", 3.0), ("a_det", 0.3), ("a_spr", 0.4), ("a_lvl", 0.8),
            ("sub_on", 1.0), ("sub_lvl", 0.55),
            ("flt_cut", 300.0), ("flt_res", 0.45), ("flt_drv", 0.4),
            ("src_1", 1.0), ("dst_1", 17.0), ("amt_1", 0.35), ("shp_1", 1.0), ("rate_1", 0.25), ("ret_1", 1.0),
            ("sus_1", 1.0), ("rel_1", 220.0),
            ("dst_on", 1.0), ("dst_drv", 0.45), ("dst_mix", 0.8),
        ],
    },
    Preset {
        name: "Vowel Wobble",
        category: "DnB",
        values: &[
            ("gain", 0.96),
            ("vmode", 2.0), ("glide", 25.0),
            ("a_tbl", 3.0), ("a_pos", 0.5), ("a_rnd", 0.0), ("a_lvl", 0.85),
            ("a_w1m", 9.0), ("a_w1a", 0.5),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.5), ("shp_1", 1.0), ("sync_1", 1.0), ("div_1", 2.0),
            ("sub_on", 1.0), ("sub_lvl", 0.6),
            ("flt_cut", 4000.0), ("flt_res", 0.2),
            ("sus_1", 1.0), ("rel_1", 110.0),
            ("dst_on", 1.0), ("dst_kind", 2.0), ("dst_drv", 0.25), ("dst_mix", 0.5),
        ],
    },
    Preset {
        name: "Jungle Stab",
        category: "DnB",
        values: &[
            ("a_pos", 0.5), ("a_uni", 5.0), ("a_det", 0.3), ("a_lvl", 0.7),
            ("b_on", 1.0), ("b_tbl", 0.0), ("b_pos", 0.5), ("b_lvl", 0.6), ("b_uni", 3.0), ("b_det", 0.25), ("b_semi", 7.0),
            ("sub_on", 1.0), ("sub_lvl", 0.35),
            ("flt_kind", 0.0), ("flt_cut", 2600.0), ("flt_res", 0.2), ("flt_env", 0.45),
            ("dec_2", 180.0),
            ("dec_1", 270.0), ("sus_1", 0.0), ("rel_1", 150.0),
            ("rev_on", 1.0), ("rev_size", 0.6), ("rev_mix", 0.3),
        ],
    },
    Preset {
        name: "Liquid Rhodes",
        category: "DnB",
        values: &[
            ("gain", 0.69),
            ("a_tbl", 11.0), ("a_pos", 0.25), ("a_rnd", 0.0), ("a_lvl", 0.9),
            ("src_1", 7.0), ("dst_1", 1.0), ("amt_1", 0.4),
            ("flt_kind", 0.0), ("flt_cut", 6000.0), ("flt_key", 0.3),
            ("dec_1", 2200.0), ("sus_1", 0.2), ("rel_1", 500.0),
            ("cho_on", 1.0), ("cho_rate", 0.7), ("cho_mix", 0.45),
            ("dly_on", 1.0), ("dly_time", 262.0), ("dly_fb", 0.3), ("dly_mix", 0.15),
            ("rev_on", 1.0), ("rev_size", 0.8), ("rev_mix", 0.35),
        ],
    },
    Preset {
        name: "Atmos Wash",
        category: "DnB",
        values: &[
            ("gain", 0.91),
            ("a_tbl", 12.0), ("a_pos", 0.2), ("a_uni", 4.0), ("a_det", 0.2), ("a_lvl", 0.7),
            ("src_1", 1.0), ("dst_1", 1.0), ("amt_1", 0.3), ("shp_1", 6.0), ("rate_1", 0.1), ("ret_1", 0.0),
            ("noi_on", 1.0), ("noi_lvl", 0.06),
            ("flt_kind", 2.0), ("flt_cut", 300.0), ("flt_res", 0.1),
            ("atk_1", 1200.0), ("sus_1", 0.9), ("rel_1", 3000.0),
            ("dly_on", 1.0), ("dly_time", 524.0), ("dly_fb", 0.5), ("dly_mix", 0.2),
            ("rev_on", 1.0), ("rev_size", 0.95), ("rev_mix", 0.6),
        ],
    },
];

/// Reset every parameter to its default, then apply `preset` on top (if any).
pub fn apply(params: &SzParams, setter: &ParamSetter, preset: Option<&Preset>) {
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
    use crate::fx::FxChain;
    use crate::params::*;
    use crate::voice::tests::render_current;
    use crate::voice::{Engine, Snapshot};
    use std::collections::HashSet;

    #[test]
    fn every_preset_only_uses_real_ids_and_in_range_values() {
        let params = SzParams::default();
        let map = params.param_map();
        let ids: HashSet<&str> = map.iter().map(|(id, _, _)| id.as_str()).collect();
        for preset in PRESETS {
            let mut seen = HashSet::new();
            for (id, plain) in preset.values {
                assert!(ids.contains(id), "preset '{}' uses unknown parameter '{id}'", preset.name);
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
    }

    /// Apply a preset straight to an engine snapshot. Tests have no host to set parameters
    /// through, so this mirrors `Snapshot::read`; effect-rack and master values are skipped.
    fn snapshot_for(preset: &Preset) -> Snapshot {
        use nih_plug::prelude::Enum;
        let mut s = Snapshot::read(&SzParams::default(), Some(120.0));
        let mut lfo_sync = [false; 4];
        let mut lfo_div = [2usize; 4];
        let mut lfo_rate = [s.lfo[0].rate_hz, s.lfo[1].rate_hz, s.lfo[2].rate_hz, s.lfo[3].rate_hz];
        for &(id, v) in preset.values {
            let b = v >= 0.5;
            let u = v as usize;
            let (head, tail) = id.split_once('_').unwrap_or((id, ""));
            let n = tail.parse::<usize>().unwrap_or(1).saturating_sub(1);
            match (head, tail) {
                ("vmode", _) => s.mode = VoiceMode::from_index(u),
                ("poly", _) => s.nvoices = u,
                ("glide", _) => s.glide_ms = v,
                ("bend", _) => s.bend_range = v,
                ("a" | "b", _) => {
                    let o = &mut s.osc[if head == "a" { 0 } else { 1 }];
                    match tail {
                        "on" => o.on = b,
                        "tbl" => o.table = u,
                        "pos" => o.pos = v,
                        "semi" => o.semi = v,
                        "fine" => o.fine = v,
                        "lvl" => o.level = v,
                        "pan" => o.pan = v,
                        "ph" => o.phase = v,
                        "rnd" => o.rand = v,
                        "uni" => o.unison = u,
                        "det" => o.detune = v,
                        "bln" => o.blend = v,
                        "spr" => o.spread = v,
                        "w1m" => o.warp[0].0 = WarpMode::from_index(u),
                        "w1a" => o.warp[0].1 = v,
                        "w2m" => o.warp[1].0 = WarpMode::from_index(u),
                        "w2a" => o.warp[1].1 = v,
                        "flt" => o.to_filter = b,
                        _ => panic!("unmapped id {id}"),
                    }
                }
                ("sub", "on") => s.sub_on = b,
                ("sub", "shp") => s.sub_shape = SubShape::from_index(u),
                ("sub", "oct") => s.sub_oct = v as i32,
                ("sub", "lvl") => s.sub_level = v,
                ("sub", "flt") => s.sub_to_filter = b,
                ("noi", "on") => s.noise_on = b,
                ("noi", "lvl") => s.noise_level = v,
                ("noi", "flt") => s.noise_to_filter = b,
                ("flt", "on") => s.filt_on = b,
                ("flt", "kind") => s.filt_kind = FilterKind::from_index(u),
                ("flt", "cut") => s.cutoff = v,
                ("flt", "res") => s.res = v,
                ("flt", "drv") => s.drive = v,
                ("flt", "mix") => s.fmix = v,
                ("flt", "key") => s.keytrack = v,
                ("flt", "env") => s.env_amt = v,
                ("atk", _) => s.env[n].a = v,
                ("dec", _) => s.env[n].d = v,
                ("sus", _) => s.env[n].s = v,
                ("rel", _) => s.env[n].r = v,
                ("shp", _) => s.lfo[n].shape = LfoShape::from_index(u),
                ("rate", _) => lfo_rate[n] = v,
                ("sync", _) => lfo_sync[n] = b,
                ("div", _) => lfo_div[n] = u,
                ("ret", _) => s.lfo[n].retrig = b,
                ("src", _) => s.slots[n].0 = u,
                ("dst", "on") => s.dist_on = b,
                ("dst", "kind") => s.dist_kind = DistKind::from_index(u),
                ("dst", "drv") => s.dist_drive = v,
                ("dst", "mix") => s.dist_mix = v,
                ("dst", _) => s.slots[n].1 = u,
                ("amt", _) => s.slots[n].2 = v,
                ("cho" | "dly" | "rev" | "gain", _) => {}
                _ => panic!("unmapped id {id}"),
            }
        }
        for i in 0..4 {
            s.lfo[i].rate_hz = if lfo_sync[i] { 2.0 / LfoDiv::from_index(lfo_div[i]).beats() } else { lfo_rate[i] };
        }
        s
    }

    /// Every preset must make a sound at a sensible level, stay finite, and stop after release.
    #[test]
    fn every_preset_sounds_at_a_sensible_level() {
        let default_gain = util::db_to_gain(-4.0);
        let mut problems = Vec::new();
        for preset in PRESETS {
            let params = SzParams::default();
            let mut engine = Engine::new(48_000.0, &params);
            engine.snap = snapshot_for(preset);
            // Mono patches are played as a single line, poly ones as a triad.
            let mono = engine.snap.mode != VoiceMode::Poly;
            let notes: &[u8] = match (mono, preset.category == "Bass") {
                (true, true) => &[36],
                (true, false) => &[60],
                (false, true) => &[36],
                (false, false) => &[60, 64, 67],
            };
            for &n in notes {
                engine.note_on(n, 0.85);
            }
            let l = render_current(&mut engine, 1.5);
            assert!(l.iter().all(|v| v.is_finite()), "preset '{}' is not finite", preset.name);
            let gain = preset.values.iter().find(|(k, _)| *k == "gain").map(|(_, v)| *v).unwrap_or(default_gain);
            let peak = l.iter().fold(0.0f32, |m, v| m.max(v.abs())) * gain;
            let rms = (l.iter().map(|v| v * v).sum::<f32>() / l.len() as f32).sqrt() * gain;
            println!("{:<20} peak {:>5.2}  rms {:>5.3}  (master {:.2})", preset.name, peak, rms, gain);
            // After the master gain every preset should land in the same loudness window.
            if !(0.3..=0.85).contains(&peak) {
                problems.push(format!("'{}' peaks at {peak:.2} (want 0.30..0.85)", preset.name));
            }
            if rms < 0.02 {
                problems.push(format!("'{}' has almost no energy (rms {rms:.3})", preset.name));
            }
            for &n in notes {
                engine.note_off(n);
            }
            render_current(&mut engine, 16.0);
            if engine.active_voices() != 0 {
                problems.push(format!("'{}' never stops", preset.name));
            }
        }
        assert!(problems.is_empty(), "preset problems:\n{}", problems.join("\n"));
    }

    /// The effect rack must stay finite and bounded with everything switched on.
    #[test]
    fn effects_stay_bounded() {
        let params = SzParams::default();
        let mut engine = Engine::new(48_000.0, &params);
        let mut fx = FxChain::new(48_000.0);
        engine.note_on(48, 1.0);
        let mut l = render_current(&mut engine, 1.0);
        let mut r = l.clone();
        fx.process(&params, &mut l, &mut r);
        assert!(l.iter().chain(r.iter()).all(|v| v.is_finite() && v.abs() < 4.0));
    }

    #[test]
    fn preset_names_are_unique_and_the_first_is_the_default() {
        let names: HashSet<&str> = PRESETS.iter().map(|p| p.name).collect();
        assert_eq!(names.len(), PRESETS.len());
        assert!(PRESETS[0].values.is_empty());
        assert!(PRESETS.len() >= 24);
    }
}
