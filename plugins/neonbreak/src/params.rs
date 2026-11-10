//! Every automatable parameter of Neon Break.

use crate::bank::{BankCell, SLOTS};
use crate::sample::SampleCell;
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

pub const STEPS: usize = 16;
pub const VOICES: usize = 8;
pub const EDITOR_W: u32 = 1160;
pub const EDITOR_H: u32 = 792;
/// Height of the keyboard strip (and its gap) at the bottom of the standalone window.
pub const KEYBOARD_H: u32 = 70;

// ---------------------------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------------------------

/// When the built-in patterns play.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SeqMode {
    /// Only while a pattern key (or fill key) is held: the arrangement lives in the piano roll.
    #[name = "Pattern keys"]
    Keys,
    /// Whenever the host plays, using the selected slot; pattern keys still override it.
    #[name = "Always on"]
    Always,
    /// Never: a plain drum module that only plays the drum notes it receives.
    #[name = "Off"]
    Off,
}

/// Where the kick goes, for sidechaining.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum KickOut {
    /// In the main mix only; the second output stays silent.
    #[name = "In mix"]
    Mix,
    /// In the main mix, plus a copy on the second output to key a sidechain with.
    #[name = "Mix + copy"]
    Both,
    /// Only on the second output, to mix and process the kick on its own track.
    #[name = "Separate"]
    Separate,
}

/// A fill that replaces the pattern while it is held.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fill {
    #[name = "Off"]
    Off,
    #[name = "Snare Rush"]
    SnareRush,
    #[name = "Tom Fill"]
    TomFill,
    #[name = "Hat Roll"]
    HatRoll,
    #[name = "Build"]
    Build,
}

/// How long the reverb stays open after a hit before the gate slams shut.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gate {
    #[name = "Open (no gate)"]
    Open,
    #[name = "1/32"]
    ThirtySecond,
    #[name = "1/16"]
    Sixteenth,
    #[name = "1/8"]
    Eighth,
    #[name = "1/4"]
    Quarter,
    #[name = "1/2"]
    Half,
}

impl Gate {
    /// Gate length in quarter notes; `None` leaves the reverb ungated.
    pub fn beats(self) -> Option<f32> {
        match self {
            Gate::Open => None,
            Gate::ThirtySecond => Some(0.125),
            Gate::Sixteenth => Some(0.25),
            Gate::Eighth => Some(0.5),
            Gate::Quarter => Some(1.0),
            Gate::Half => Some(2.0),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The eight drums
// ---------------------------------------------------------------------------------------------

/// Static description of one drum: ids, labels, tooltips and default settings.
pub struct VoiceInfo {
    /// Parameter id prefix (`k` gives `k_lvl`, `k_v_1`...).
    pub prefix: &'static str,
    pub name: &'static str,
    /// Short upper-case name for the editor.
    pub label: &'static str,
    /// MIDI note that plays this drum (General MIDI).
    pub note: u8,
    /// Labels and explanations of the two character knobs, which differ per drum.
    pub a: (&'static str, &'static str),
    pub b: (&'static str, &'static str),
    pub tone_tip: &'static str,
    /// level, pan, tune, decay, tone, a, b, reverb send
    pub defaults: [f32; 8],
}

pub const VOICE_INFO: [VoiceInfo; VOICES] = [
    VoiceInfo {
        prefix: "k",
        name: "Kick",
        label: "KICK",
        note: 36,
        a: ("PUNCH", "How far the pitch drops at the start of the hit. Low = soft, round 808; high = hard, knocking DnB kick."),
        b: ("CLICK", "Level of the short noise tick at the very start, which helps the kick cut through a busy mix."),
        tone_tip: "Saturates the kick: low is a pure sine sub, high adds harmonics so it is audible on small speakers.",
        defaults: [0.85, 0.0, 0.0, 0.45, 0.3, 0.5, 0.3, 0.0],
    },
    VoiceInfo {
        prefix: "s",
        name: "Snare",
        label: "SNARE",
        note: 38,
        a: ("SNAP", "Balance between the drum's tonal body (low) and its noisy snare wires (high)."),
        b: ("BEND", "Downward pitch sweep of the body. Turn it up for the 'pew' of an 80s electronic snare."),
        tone_tip: "Brightness of the noise part: low is dark and fat, high is crisp and airy.",
        defaults: [0.8, 0.0, 0.0, 0.45, 0.5, 0.6, 0.3, 0.5],
    },
    VoiceInfo {
        prefix: "c",
        name: "Clap",
        label: "CLAP",
        note: 39,
        a: ("SPREAD", "Time between the individual hand claps. Low = one tight snap, high = a loose group of clappers."),
        b: ("FOCUS", "How narrow the clap's frequency band is. High values sound thinner and more electronic."),
        tone_tip: "Pitch of the clap's noise band.",
        defaults: [0.6, 0.0, 0.0, 0.4, 0.5, 0.4, 0.4, 0.4],
    },
    VoiceInfo {
        prefix: "hc",
        name: "Closed Hat",
        label: "CL HAT",
        note: 42,
        a: ("METAL", "Blend between plain noise (low) and a cluster of detuned square waves (high), the classic drum-machine hat."),
        b: ("RING", "Resonance of the hat's filter: adds a whistling, pitched edge."),
        tone_tip: "Removes lows below this point: higher = thinner and brighter.",
        defaults: [0.55, 0.15, 0.0, 0.3, 0.6, 0.7, 0.3, 0.0],
    },
    VoiceInfo {
        prefix: "ho",
        name: "Open Hat",
        label: "OP HAT",
        note: 46,
        a: ("METAL", "Blend between plain noise (low) and a cluster of detuned square waves (high)."),
        b: ("RING", "Resonance of the hat's filter: adds a whistling, pitched edge."),
        tone_tip: "Removes lows below this point: higher = thinner and brighter.",
        defaults: [0.5, 0.15, 0.0, 0.4, 0.6, 0.7, 0.3, 0.1],
    },
    VoiceInfo {
        prefix: "th",
        name: "High Tom",
        label: "TOM HI",
        note: 50,
        a: ("BEND", "Downward pitch sweep. Low = a natural drum; high = the 'dooom' of 80s hexagonal electronic toms."),
        b: ("NOISE", "Level of the stick and shell noise at the start of the hit."),
        tone_tip: "Saturates the tom, from a pure tone to a buzzier, more aggressive one.",
        defaults: [0.7, -0.3, 0.0, 0.45, 0.35, 0.2, 0.45, 0.2],
    },
    VoiceInfo {
        prefix: "tl",
        name: "Low Tom",
        label: "TOM LO",
        note: 45,
        a: ("BEND", "Downward pitch sweep. Low = a natural drum; high = the 'dooom' of 80s hexagonal electronic toms."),
        b: ("NOISE", "Level of the stick and shell noise at the start of the hit."),
        tone_tip: "Saturates the tom, from a pure tone to a buzzier, more aggressive one.",
        defaults: [0.7, 0.3, 0.0, 0.45, 0.35, 0.2, 0.45, 0.2],
    },
    VoiceInfo {
        prefix: "cy",
        name: "Cymbal",
        label: "CYMBAL",
        note: 49,
        a: ("METAL", "Blend between a noisy wash (low) and a metallic, pitched shimmer (high)."),
        b: ("BELL", "Adds a ringing ping, turning the crash into more of a ride."),
        tone_tip: "Removes lows below this point: higher = thinner and brighter.",
        defaults: [0.45, -0.2, 0.0, 0.5, 0.5, 0.7, 0.2, 0.2],
    },
];

// ---------------------------------------------------------------------------------------------
// Parameter constructors
// ---------------------------------------------------------------------------------------------

fn pct(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 100.0)))
        .with_string_to_value(Arc::new(|s| {
            s.trim().trim_end_matches('%').trim().parse::<f32>().ok().map(|v| v / 100.0)
        }))
}

fn pan(name: impl Into<String>, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: -1.0, max: 1.0 })
        .with_value_to_string(Arc::new(|v| {
            if v.abs() < 0.005 {
                "C".to_string()
            } else if v < 0.0 {
                format!("{:.0} L", -v * 100.0)
            } else {
                format!("{:.0} R", v * 100.0)
            }
        }))
}

#[derive(Params)]
pub struct VoiceParams {
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "tune"]
    pub tune: FloatParam,
    #[id = "dec"]
    pub decay: FloatParam,
    #[id = "tone"]
    pub tone: FloatParam,
    #[id = "a"]
    pub a: FloatParam,
    #[id = "b"]
    pub b: FloatParam,
    #[id = "send"]
    pub send: FloatParam,
    #[id = "mute"]
    pub mute: BoolParam,
    /// Blend between the synthesized drum (0) and the loaded sample (1).
    #[id = "smp"]
    pub sample_mix: FloatParam,
}

impl VoiceParams {
    fn new(index: usize) -> Self {
        let info = &VOICE_INFO[index];
        let n = info.name;
        let d = info.defaults;
        Self {
            level: pct(format!("{n} Level"), d[0]),
            pan: pan(format!("{n} Pan"), d[1]),
            tune: FloatParam::new(format!("{n} Tune"), d[2], FloatRange::Linear { min: -12.0, max: 12.0 })
                .with_unit(" st")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            decay: pct(format!("{n} Decay"), d[3]),
            tone: pct(format!("{n} Tone"), d[4]),
            a: pct(format!("{n} {}", info.a.0), d[5]),
            b: pct(format!("{n} {}", info.b.0), d[6]),
            send: pct(format!("{n} Reverb Send"), d[7]),
            mute: BoolParam::new(format!("{n} Mute"), false),
            sample_mix: pct(format!("{n} Sample Mix"), 1.0),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Top level
// ---------------------------------------------------------------------------------------------

#[derive(Params)]
pub struct NeonParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    /// Index of the last loaded factory preset (only used to label the preset browser).
    #[persist = "preset"]
    pub preset: Arc<std::sync::atomic::AtomicI32>,

    #[id = "gain"]
    pub gain: FloatParam,

    /// The twelve pattern slots (not automatable; saved with the project).
    #[persist = "bank"]
    pub bank: BankCell,
    /// File paths of the loaded samples, one per drum.
    #[persist = "samples"]
    pub samples: SampleCell,

    // Sequencer
    #[id = "mode"]
    pub mode: EnumParam<SeqMode>,
    /// Pattern slot shown in the editor, and the one "Always on" and PLAY use.
    #[id = "slot"]
    pub slot: IntParam,
    #[id = "kick_out"]
    pub kick_out: EnumParam<KickOut>,
    #[id = "swing"]
    pub swing: FloatParam,
    #[id = "human"]
    pub human: FloatParam,
    #[id = "fill"]
    pub fill: EnumParam<Fill>,
    /// Tempo of the built-in clock, used by the standalone app (a DAW provides its own tempo).
    #[id = "bpm"]
    pub bpm: FloatParam,

    // Gated reverb
    #[id = "rev_size"]
    pub rev_size: FloatParam,
    #[id = "rev_gate"]
    pub rev_gate: EnumParam<Gate>,
    #[id = "rev_tone"]
    pub rev_tone: FloatParam,
    #[id = "rev_lvl"]
    pub rev_level: FloatParam,

    // Drum bus
    #[id = "bus_punch"]
    pub punch: FloatParam,
    #[id = "bus_drive"]
    pub drive: FloatParam,
    #[id = "bus_glue"]
    pub glue: FloatParam,
    #[id = "bus_duck"]
    pub duck: FloatParam,

    #[nested(id_prefix = "k", group = "Kick")]
    pub kick: VoiceParams,
    #[nested(id_prefix = "s", group = "Snare")]
    pub snare: VoiceParams,
    #[nested(id_prefix = "c", group = "Clap")]
    pub clap: VoiceParams,
    #[nested(id_prefix = "hc", group = "Closed Hat")]
    pub hat_closed: VoiceParams,
    #[nested(id_prefix = "ho", group = "Open Hat")]
    pub hat_open: VoiceParams,
    #[nested(id_prefix = "th", group = "High Tom")]
    pub tom_hi: VoiceParams,
    #[nested(id_prefix = "tl", group = "Low Tom")]
    pub tom_lo: VoiceParams,
    #[nested(id_prefix = "cy", group = "Cymbal")]
    pub cymbal: VoiceParams,
}

impl NeonParams {
    /// The eight drums in pattern-row order (matches `VOICE_INFO`).
    pub fn voices(&self) -> [&VoiceParams; VOICES] {
        [
            &self.kick,
            &self.snare,
            &self.clap,
            &self.hat_closed,
            &self.hat_open,
            &self.tom_hi,
            &self.tom_lo,
            &self.cymbal,
        ]
    }
}

impl Default for NeonParams {
    fn default() -> Self {
        Self {
            // The on-screen keyboard strip only exists in the standalone app.
            editor_state: EguiState::from_size(
                EDITOR_W,
                if plugin_common::midi::is_standalone() { EDITOR_H } else { EDITOR_H - KEYBOARD_H },
            ),
            preset: Arc::new(std::sync::atomic::AtomicI32::new(0)),

            gain: FloatParam::new(
                "Output",
                util::db_to_gain(-3.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-60.0),
                    max: util::db_to_gain(6.0),
                    factor: FloatRange::gain_skew_factor(-60.0, 6.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            bank: BankCell::default(),
            samples: SampleCell::default(),

            mode: EnumParam::new("Pattern Mode", SeqMode::Keys),
            slot: IntParam::new("Pattern Slot", 1, IntRange::Linear { min: 1, max: SLOTS as i32 }),
            kick_out: EnumParam::new("Kick Output", KickOut::Mix),
            swing: pct("Swing", 0.0),
            human: pct("Humanize", 0.15),
            fill: EnumParam::new("Fill", Fill::Off),
            bpm: FloatParam::new("Tempo (standalone)", 174.0, FloatRange::Linear { min: 60.0, max: 200.0 })
                .with_unit(" BPM")
                .with_value_to_string(formatters::v2s_f32_rounded(0)),

            rev_size: pct("Reverb Size", 0.6),
            rev_gate: EnumParam::new("Reverb Gate", Gate::Eighth),
            rev_tone: pct("Reverb Tone", 0.6),
            rev_level: pct("Reverb Level", 0.5),

            punch: pct("Bus Punch", 0.25),
            drive: pct("Bus Drive", 0.2),
            glue: pct("Bus Glue", 0.3),
            duck: pct("Kick Duck", 0.0),

            kick: VoiceParams::new(0),
            snare: VoiceParams::new(1),
            clap: VoiceParams::new(2),
            hat_closed: VoiceParams::new(3),
            hat_open: VoiceParams::new(4),
            tom_hi: VoiceParams::new(5),
            tom_lo: VoiceParams::new(6),
            cymbal: VoiceParams::new(7),
        }
    }
}
