//! Every automatable parameter of the Wub Machine.

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

pub const STEPS: usize = 16;
pub const EDITOR_W: u32 = 1160;
pub const EDITOR_H: u32 = 836;
/// Height of the keyboard strip (and its gap) at the bottom of the standalone window.
pub const KEYBOARD_H: u32 = 70;

// ---------------------------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------------------------

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wave1 {
    #[name = "Saw"]
    Saw,
    #[name = "Square"]
    Square,
    #[name = "Pulse (PWM)"]
    Pulse,
    #[name = "Triangle"]
    Triangle,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wave2 {
    #[name = "Saw"]
    Saw,
    #[name = "Square"]
    Square,
    #[name = "Sine"]
    Sine,
    #[name = "Triangle"]
    Triangle,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubWave {
    #[name = "Sine"]
    Sine,
    #[name = "Triangle"]
    Triangle,
    #[name = "Square"]
    Square,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterKind {
    #[name = "Low 24 (Ladder)"]
    Low24,
    #[name = "Low 12"]
    Low12,
    #[name = "Band 12"]
    Band12,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DistKind {
    #[name = "Off"]
    Off,
    #[name = "Soft"]
    Soft,
    #[name = "Hard Clip"]
    Hard,
    #[name = "Wavefold"]
    Fold,
    #[name = "Tube"]
    Tube,
    #[name = "Rectify"]
    Rectify,
    #[name = "Bit Crush"]
    Crush,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LfoShape {
    #[name = "Sine"]
    Sine,
    #[name = "Triangle"]
    Triangle,
    #[name = "Saw Up"]
    SawUp,
    #[name = "Saw Down"]
    SawDown,
    #[name = "Square"]
    Square,
    #[name = "Soft Square"]
    SoftSquare,
    #[name = "Half Sine"]
    HalfSine,
    #[name = "Random"]
    Random,
}

/// How an LFO decides where it is in its cycle.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LfoTrig {
    /// Restart at every new note (tight, repeatable wubs).
    #[name = "Note"]
    Note,
    /// Locked to the host's beat grid (falls back to Note when the host is stopped).
    #[name = "Grid"]
    Grid,
    /// Free running.
    #[name = "Free"]
    Free,
}

/// Musical lengths of one LFO cycle. `beats()` is measured in quarter notes.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum RateDiv {
    #[name = "1 bar"]
    Bar,
    #[name = "1/2"]
    Half,
    #[name = "1/2T"]
    HalfT,
    #[name = "1/4."]
    QuarterDot,
    #[name = "1/4"]
    Quarter,
    #[name = "1/4T"]
    QuarterT,
    #[name = "1/8."]
    EighthDot,
    #[name = "1/8"]
    Eighth,
    #[name = "1/8T"]
    EighthT,
    #[name = "1/16."]
    SixteenthDot,
    #[name = "1/16"]
    Sixteenth,
    #[name = "1/16T"]
    SixteenthT,
    #[name = "1/32"]
    ThirtySecond,
}

impl RateDiv {
    pub fn beats(self) -> f32 {
        match self {
            RateDiv::Bar => 4.0,
            RateDiv::Half => 2.0,
            RateDiv::HalfT => 4.0 / 3.0,
            RateDiv::QuarterDot => 1.5,
            RateDiv::Quarter => 1.0,
            RateDiv::QuarterT => 2.0 / 3.0,
            RateDiv::EighthDot => 0.75,
            RateDiv::Eighth => 0.5,
            RateDiv::EighthT => 1.0 / 3.0,
            RateDiv::SixteenthDot => 0.375,
            RateDiv::Sixteenth => 0.25,
            RateDiv::SixteenthT => 1.0 / 6.0,
            RateDiv::ThirtySecond => 0.125,
        }
    }
}

/// Length of one pattern step.
#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum StepLen {
    #[name = "1/16"]
    Sixteenth,
    #[name = "1/8"]
    Eighth,
    #[name = "1/4"]
    Quarter,
    #[name = "1/2"]
    Half,
    #[name = "1 bar"]
    Bar,
}

impl StepLen {
    pub fn beats(self) -> f32 {
        match self {
            StepLen::Sixteenth => 0.25,
            StepLen::Eighth => 0.5,
            StepLen::Quarter => 1.0,
            StepLen::Half => 2.0,
            StepLen::Bar => 4.0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Parameter constructors
// ---------------------------------------------------------------------------------------------

fn pct(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 100.0)))
        .with_string_to_value(Arc::new(|s| {
            s.trim().trim_end_matches('%').trim().parse::<f32>().ok().map(|v| v / 100.0)
        }))
}

fn bipolar(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: -1.0, max: 1.0 })
        .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 100.0)))
        .with_string_to_value(Arc::new(|s| {
            s.trim().trim_end_matches('%').trim().parse::<f32>().ok().map(|v| v / 100.0)
        }))
        .with_unit(" %")
}

fn time_ms(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-2.2) })
        .with_unit(" ms")
        .with_value_to_string(formatters::v2s_f32_rounded(1))
}

fn flag(name: &str, default: bool) -> BoolParam {
    BoolParam::new(name, default)
}

// ---------------------------------------------------------------------------------------------
// Parameter groups
// ---------------------------------------------------------------------------------------------

#[derive(Params)]
pub struct Osc1Params {
    #[id = "wave"]
    pub wave: EnumParam<Wave1>,
    #[id = "pwm"]
    pub pwm: FloatParam,
    #[id = "uni"]
    pub voices: IntParam,
    #[id = "det"]
    pub detune: FloatParam,
    #[id = "spr"]
    pub spread: FloatParam,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "rnd"]
    pub rand: FloatParam,
}

#[derive(Params)]
pub struct Osc2Params {
    #[id = "wave"]
    pub wave: EnumParam<Wave2>,
    #[id = "semi"]
    pub semi: IntParam,
    #[id = "fine"]
    pub fine: FloatParam,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "fm"]
    pub fm: FloatParam,
}

#[derive(Params)]
pub struct SubParams {
    #[id = "wave"]
    pub wave: EnumParam<SubWave>,
    #[id = "oct"]
    pub octave: IntParam,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "flt"]
    pub to_filter: BoolParam,
}

#[derive(Params)]
pub struct FilterParams {
    #[id = "kind"]
    pub kind: EnumParam<FilterKind>,
    #[id = "cut"]
    pub cutoff: FloatParam,
    #[id = "res"]
    pub res: FloatParam,
    #[id = "drv"]
    pub drive: FloatParam,
    #[id = "key"]
    pub keytrack: FloatParam,
    #[id = "env"]
    pub env_amt: FloatParam,
}

#[derive(Params)]
pub struct FormantParams {
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "morph"]
    pub morph: FloatParam,
    #[id = "q"]
    pub q: FloatParam,
}

#[derive(Params)]
pub struct DistParams {
    #[id = "kind"]
    pub kind: EnumParam<DistKind>,
    #[id = "drv"]
    pub drive: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
}

#[derive(Params)]
pub struct ToneParams {
    #[id = "lo"]
    pub low_cut: FloatParam,
    #[id = "hi"]
    pub high_cut: FloatParam,
    #[id = "ott"]
    pub ott: FloatParam,
}

#[derive(Params)]
pub struct EnvParams {
    #[id = "atk"]
    pub attack: FloatParam,
    #[id = "dec"]
    pub decay: FloatParam,
    #[id = "sus"]
    pub sustain: FloatParam,
    #[id = "rel"]
    pub release: FloatParam,
}

impl EnvParams {
    fn new(a: f32, d: f32, s: f32, r: f32) -> Self {
        Self {
            attack: time_ms("Attack", a, 0.5, 5_000.0),
            decay: time_ms("Decay", d, 1.0, 5_000.0),
            sustain: pct("Sustain", s),
            release: time_ms("Release", r, 1.0, 5_000.0),
        }
    }
}

#[derive(Params)]
pub struct LfoParams {
    #[id = "shape"]
    pub shape: EnumParam<LfoShape>,
    #[id = "sync"]
    pub sync: BoolParam,
    #[id = "div"]
    pub div: EnumParam<RateDiv>,
    #[id = "hz"]
    pub hz: FloatParam,
    #[id = "trig"]
    pub trig: EnumParam<LfoTrig>,
    #[id = "phase"]
    pub phase: FloatParam,
    #[id = "smooth"]
    pub smooth: FloatParam,
    #[id = "bip"]
    pub bipolar: BoolParam,
    // Modulation depths (what the LFO moves)
    #[id = "cut"]
    pub cutoff: FloatParam,
    #[id = "res"]
    pub res: FloatParam,
    #[id = "drv"]
    pub drive: FloatParam,
    #[id = "fmt"]
    pub formant: FloatParam,
    #[id = "fm"]
    pub fm: FloatParam,
    #[id = "pwm"]
    pub pwm: FloatParam,
    #[id = "pit"]
    pub pitch: FloatParam,
    #[id = "amp"]
    pub amp: FloatParam,
}

impl LfoParams {
    fn new(shape: LfoShape, div: RateDiv, hz: f32, trig: LfoTrig, sync: bool) -> Self {
        Self {
            shape: EnumParam::new("Shape", shape),
            sync: flag("Tempo Sync", sync),
            div: EnumParam::new("Rate", div),
            hz: FloatParam::new(
                "Rate (Hz)",
                hz,
                FloatRange::Skewed { min: 0.03, max: 40.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            trig: EnumParam::new("Restart", trig),
            phase: pct("Phase", 0.0),
            smooth: pct("Smooth", 0.1),
            bipolar: flag("Bipolar", false),
            cutoff: bipolar("To Cutoff", 0.0),
            res: bipolar("To Reso", 0.0),
            drive: bipolar("To Drive", 0.0),
            formant: bipolar("To Formant", 0.0),
            fm: bipolar("To FM", 0.0),
            pwm: bipolar("To PWM", 0.0),
            pitch: bipolar("To Pitch", 0.0),
            amp: bipolar("To Volume", 0.0),
        }
    }
}

#[derive(Params)]
pub struct StepParams {
    #[id = "son"]
    pub on: BoolParam,
    #[id = "srate"]
    pub rate: EnumParam<RateDiv>,
    #[id = "sdep"]
    pub depth: FloatParam,
    #[id = "ssemi"]
    pub semi: IntParam,
}

#[derive(Params)]
pub struct PatternParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "len"]
    pub len: IntParam,
    #[id = "step"]
    pub step: EnumParam<StepLen>,
    #[nested(array, group = "Step")]
    pub steps: [StepParams; STEPS],
}

// ---------------------------------------------------------------------------------------------
// Top level
// ---------------------------------------------------------------------------------------------

#[derive(Params)]
pub struct WubParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    /// Index of the last loaded factory preset (only used to label the preset browser).
    #[persist = "preset"]
    pub preset: Arc<std::sync::atomic::AtomicI32>,

    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "vel"]
    pub vel_sens: FloatParam,
    #[id = "glide"]
    pub glide: FloatParam,
    #[id = "legato"]
    pub legato: BoolParam,
    #[id = "bend"]
    pub bend_range: IntParam,
    #[id = "wheel"]
    pub wheel_amt: FloatParam,

    #[nested(id_prefix = "o1", group = "Osc 1")]
    pub osc1: Osc1Params,
    #[nested(id_prefix = "o2", group = "Osc 2")]
    pub osc2: Osc2Params,
    #[nested(id_prefix = "sub", group = "Sub")]
    pub sub: SubParams,
    #[id = "noise"]
    pub noise: FloatParam,

    #[nested(id_prefix = "flt", group = "Filter")]
    pub filter: FilterParams,
    #[nested(id_prefix = "fmt", group = "Formant")]
    pub formant: FormantParams,
    #[nested(id_prefix = "da", group = "Distortion A")]
    pub dist_a: DistParams,
    #[nested(id_prefix = "db", group = "Distortion B")]
    pub dist_b: DistParams,
    #[nested(id_prefix = "tone", group = "Tone")]
    pub tone: ToneParams,

    #[nested(id_prefix = "amp", group = "Amp Envelope")]
    pub amp_env: EnvParams,
    #[nested(id_prefix = "fenv", group = "Filter Envelope")]
    pub filt_env: EnvParams,

    #[nested(id_prefix = "l1", group = "LFO 1 (Wub)")]
    pub lfo1: LfoParams,
    #[nested(id_prefix = "l2", group = "LFO 2 (Growl)")]
    pub lfo2: LfoParams,

    #[nested(id_prefix = "pat", group = "Pattern")]
    pub pattern: PatternParams,
}

impl Default for WubParams {
    fn default() -> Self {
        let steps: Vec<StepParams> = (0..STEPS)
            .map(|_| StepParams {
                on: flag("On", true),
                rate: EnumParam::new("Rate", RateDiv::Eighth),
                depth: pct("Depth", 1.0),
                semi: IntParam::new("Interval", 0, IntRange::Linear { min: -24, max: 24 }),
            })
            .collect();
        let steps: [StepParams; STEPS] = match steps.try_into() {
            Ok(a) => a,
            Err(_) => unreachable!(),
        };

        let mut lfo1 = LfoParams::new(LfoShape::Sine, RateDiv::Eighth, 2.0, LfoTrig::Note, true);
        lfo1.cutoff = bipolar("To Cutoff", 0.65);
        let lfo2 = LfoParams::new(LfoShape::Triangle, RateDiv::Quarter, 0.35, LfoTrig::Free, false);

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
            vel_sens: pct("Velocity", 0.5),
            glide: time_ms("Glide", 0.0, 0.0, 1500.0),
            legato: flag("Legato", false),
            bend_range: IntParam::new("Bend Range", 2, IntRange::Linear { min: 0, max: 24 }),
            wheel_amt: pct("Wheel > Depth", 0.0),

            osc1: Osc1Params {
                wave: EnumParam::new("Wave", Wave1::Saw),
                pwm: pct("PWM", 0.5),
                voices: IntParam::new("Unison", 3, IntRange::Linear { min: 1, max: 7 }),
                detune: pct("Detune", 0.25),
                spread: pct("Stereo", 0.5),
                level: pct("Level", 0.8),
                rand: pct("Rand Phase", 0.0),
            },
            osc2: Osc2Params {
                wave: EnumParam::new("Wave", Wave2::Square),
                semi: IntParam::new("Semitone", 0, IntRange::Linear { min: -24, max: 24 }),
                fine: FloatParam::new("Fine", 7.0, FloatRange::Linear { min: -100.0, max: 100.0 })
                    .with_unit(" ct")
                    .with_value_to_string(formatters::v2s_f32_rounded(1)),
                level: pct("Level", 0.4),
                fm: pct("FM > Osc 1", 0.0),
            },
            sub: SubParams {
                wave: EnumParam::new("Wave", SubWave::Sine),
                octave: IntParam::new("Octave", -1, IntRange::Linear { min: -2, max: -1 }),
                level: pct("Level", 0.5),
                to_filter: flag("Through Filter", false),
            },
            noise: pct("Noise", 0.0),

            filter: FilterParams {
                kind: EnumParam::new("Type", FilterKind::Low24),
                cutoff: FloatParam::new(
                    "Cutoff",
                    150.0,
                    FloatRange::Skewed { min: 20.0, max: 12_000.0, factor: FloatRange::skew_factor(-2.0) },
                )
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
                .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
                res: pct("Resonance", 0.5),
                drive: pct("Drive", 0.15),
                keytrack: pct("Key Track", 0.4),
                env_amt: bipolar("Env Amount", 0.0),
            },
            formant: FormantParams { mix: pct("Mix", 0.0), morph: pct("Vowel", 0.3), q: pct("Q", 0.5) },
            dist_a: DistParams { kind: EnumParam::new("Type", DistKind::Soft), drive: pct("Drive", 0.4), mix: pct("Mix", 1.0) },
            dist_b: DistParams { kind: EnumParam::new("Type", DistKind::Off), drive: pct("Drive", 0.3), mix: pct("Mix", 1.0) },
            tone: ToneParams {
                low_cut: FloatParam::new(
                    "Low Cut",
                    25.0,
                    FloatRange::Skewed { min: 20.0, max: 500.0, factor: FloatRange::skew_factor(-1.5) },
                )
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
                high_cut: FloatParam::new(
                    "High Cut",
                    20_000.0,
                    FloatRange::Skewed { min: 800.0, max: 20_000.0, factor: FloatRange::skew_factor(-1.5) },
                )
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
                .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
                ott: pct("OTT", 0.0),
            },

            amp_env: EnvParams::new(1.0, 200.0, 1.0, 120.0),
            filt_env: EnvParams::new(15.0, 250.0, 0.0, 100.0),

            lfo1,
            lfo2,

            pattern: PatternParams {
                on: flag("Pattern", false),
                len: IntParam::new("Length", 8, IntRange::Linear { min: 1, max: STEPS as i32 }),
                step: EnumParam::new("Step Size", StepLen::Sixteenth),
                steps,
            },
        }
    }
}
