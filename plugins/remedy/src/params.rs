//! Every automatable parameter of Remedy.

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------------------------

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum WtId {
    #[name = "Basic Shapes"]
    Basic,
    #[name = "PWM Sweep"]
    Pwm,
    #[name = "Saw Resonance"]
    SawRes,
    #[name = "Vowels"]
    Vowels,
    #[name = "FM Bell"]
    FmBell,
    #[name = "Organ"]
    Organ,
    #[name = "Spectral Chaos"]
    Chaos,
    #[name = "Metallic"]
    Metallic,
    #[name = "Sync Sweep"]
    SyncSweep,
    #[name = "Growl"]
    Growl,
    #[name = "Glass"]
    Glass,
    #[name = "E-Piano"]
    EPiano,
    #[name = "Harmonic Series"]
    HarmonicSeries,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum WarpMode {
    #[name = "Off"]
    Off,
    #[name = "Sync"]
    Sync,
    #[name = "Bend +"]
    BendPlus,
    #[name = "Bend -"]
    BendMinus,
    #[name = "Asym"]
    Asym,
    #[name = "Mirror"]
    Mirror,
    #[name = "Twist"]
    Twist,
    #[name = "FM (self)"]
    FmSelf,
    #[name = "Fold"]
    Fold,
    #[name = "Drive"]
    Drive,
    #[name = "Crush"]
    Crush,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubShape {
    #[name = "Sine"]
    Sine,
    #[name = "Triangle"]
    Triangle,
    #[name = "Saw"]
    Saw,
    #[name = "Square"]
    Square,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterKind {
    #[name = "Low 12"]
    Lp12,
    #[name = "Low 24 (Ladder)"]
    Lp24,
    #[name = "High 12"]
    Hp12,
    #[name = "Band 12"]
    Bp12,
    #[name = "Notch"]
    Notch,
    #[name = "Peak"]
    Peak,
    #[name = "Comb +"]
    CombPlus,
    #[name = "Comb -"]
    CombMinus,
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
    #[name = "Sample & Hold"]
    SampleHold,
    #[name = "Smooth Random"]
    SmoothRandom,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LfoDiv {
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
    #[name = "2 bars"]
    TwoBars,
    #[name = "4 bars"]
    FourBars,
}

impl LfoDiv {
    /// Length of one LFO cycle in quarter-note beats.
    pub fn beats(self) -> f32 {
        match self {
            LfoDiv::Sixteenth => 0.25,
            LfoDiv::Eighth => 0.5,
            LfoDiv::Quarter => 1.0,
            LfoDiv::Half => 2.0,
            LfoDiv::Bar => 4.0,
            LfoDiv::TwoBars => 8.0,
            LfoDiv::FourBars => 16.0,
        }
    }
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VoiceMode {
    #[name = "Poly"]
    Poly,
    #[name = "Mono"]
    Mono,
    #[name = "Legato"]
    Legato,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DistKind {
    #[name = "Soft"]
    Soft,
    #[name = "Hard Clip"]
    Hard,
    #[name = "Wavefold"]
    Fold,
    #[name = "Bit Crush"]
    Crush,
}

pub const MOD_SRC_COUNT: usize = 15;

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModSrc {
    #[name = "-"]
    None,
    #[name = "LFO 1"]
    Lfo1,
    #[name = "LFO 2"]
    Lfo2,
    #[name = "LFO 3"]
    Lfo3,
    #[name = "LFO 4"]
    Lfo4,
    #[name = "Env 2"]
    Env2,
    #[name = "Env 3"]
    Env3,
    #[name = "Velocity"]
    Velocity,
    #[name = "Note"]
    Key,
    #[name = "Mod Wheel"]
    ModWheel,
    #[name = "Aftertouch"]
    Aftertouch,
    #[name = "Macro 1"]
    Macro1,
    #[name = "Macro 2"]
    Macro2,
    #[name = "Macro 3"]
    Macro3,
    #[name = "Macro 4"]
    Macro4,
}

pub const MOD_DST_COUNT: usize = 21;

#[derive(Enum, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModDst {
    #[name = "-"]
    None,
    #[name = "A Position"]
    APos,
    #[name = "B Position"]
    BPos,
    #[name = "A Pitch"]
    APitch,
    #[name = "B Pitch"]
    BPitch,
    #[name = "A Level"]
    ALevel,
    #[name = "B Level"]
    BLevel,
    #[name = "A Pan"]
    APan,
    #[name = "B Pan"]
    BPan,
    #[name = "A Warp 1"]
    AWarp1,
    #[name = "A Warp 2"]
    AWarp2,
    #[name = "B Warp 1"]
    BWarp1,
    #[name = "B Warp 2"]
    BWarp2,
    #[name = "A Detune"]
    ADetune,
    #[name = "B Detune"]
    BDetune,
    #[name = "Sub Level"]
    SubLevel,
    #[name = "Noise Level"]
    NoiseLevel,
    #[name = "Cutoff"]
    Cutoff,
    #[name = "Resonance"]
    Resonance,
    #[name = "Filter Drive"]
    FilterDrive,
    #[name = "Amp Level"]
    Amp,
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
        .with_value_to_string(formatters::v2s_f32_rounded(2))
}

fn time_ms(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-2.2) },
    )
    .with_unit(" ms")
    .with_value_to_string(formatters::v2s_f32_rounded(1))
}

fn level(name: &str, default: f32) -> FloatParam {
    pct(name, default)
}

fn flag(name: &str, default: bool) -> BoolParam {
    BoolParam::new(name, default)
}

// ---------------------------------------------------------------------------------------------
// Parameter groups
// ---------------------------------------------------------------------------------------------

#[derive(Params)]
pub struct OscParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "tbl"]
    pub table: EnumParam<WtId>,
    #[id = "pos"]
    pub pos: FloatParam,
    #[id = "semi"]
    pub semi: IntParam,
    #[id = "fine"]
    pub fine: FloatParam,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "ph"]
    pub phase: FloatParam,
    #[id = "rnd"]
    pub rand: FloatParam,
    #[id = "uni"]
    pub unison: IntParam,
    #[id = "det"]
    pub detune: FloatParam,
    #[id = "bln"]
    pub blend: FloatParam,
    #[id = "spr"]
    pub spread: FloatParam,
    #[id = "w1m"]
    pub w1_mode: EnumParam<WarpMode>,
    #[id = "w1a"]
    pub w1_amt: FloatParam,
    #[id = "w2m"]
    pub w2_mode: EnumParam<WarpMode>,
    #[id = "w2a"]
    pub w2_amt: FloatParam,
    #[id = "flt"]
    pub to_filter: BoolParam,
}

impl OscParams {
    fn new(on: bool, table: WtId, level_default: f32) -> Self {
        Self {
            on: flag("Enable", on),
            table: EnumParam::new("Wavetable", table),
            pos: pct("WT Pos", 0.5),
            semi: IntParam::new("Semitone", 0, IntRange::Linear { min: -24, max: 24 }),
            fine: FloatParam::new("Fine", 0.0, FloatRange::Linear { min: -100.0, max: 100.0 })
                .with_unit(" ct")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            level: level("Level", level_default),
            pan: bipolar("Pan", 0.0),
            phase: pct("Phase", 0.0),
            rand: pct("Rand", 1.0),
            unison: IntParam::new("Unison", 1, IntRange::Linear { min: 1, max: 16 }),
            detune: pct("Detune", 0.25),
            blend: pct("Blend", 0.75),
            spread: pct("Stereo", 0.7),
            w1_mode: EnumParam::new("Warp 1", WarpMode::Off),
            w1_amt: pct("Warp 1 Amt", 0.0),
            w2_mode: EnumParam::new("Warp 2", WarpMode::Off),
            w2_amt: pct("Warp 2 Amt", 0.0),
            to_filter: flag("To Filter", true),
        }
    }
}

#[derive(Params)]
pub struct SubParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "shp"]
    pub shape: EnumParam<SubShape>,
    #[id = "oct"]
    pub octave: IntParam,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "flt"]
    pub to_filter: BoolParam,
}

#[derive(Params)]
pub struct NoiseParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "lvl"]
    pub level: FloatParam,
    #[id = "flt"]
    pub to_filter: BoolParam,
}

#[derive(Params)]
pub struct FilterParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "kind"]
    pub kind: EnumParam<FilterKind>,
    #[id = "cut"]
    pub cutoff: FloatParam,
    #[id = "res"]
    pub res: FloatParam,
    #[id = "drv"]
    pub drive: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "key"]
    pub keytrack: FloatParam,
    #[id = "env"]
    pub env_amt: FloatParam,
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
            attack: time_ms("Attack", a, 0.5, 10_000.0),
            decay: time_ms("Decay", d, 1.0, 10_000.0),
            sustain: pct("Sustain", s),
            release: time_ms("Release", r, 1.0, 15_000.0),
        }
    }
}

#[derive(Params)]
pub struct LfoParams {
    #[id = "shp"]
    pub shape: EnumParam<LfoShape>,
    #[id = "rate"]
    pub rate: FloatParam,
    #[id = "sync"]
    pub sync: BoolParam,
    #[id = "div"]
    pub div: EnumParam<LfoDiv>,
    #[id = "ret"]
    pub retrig: BoolParam,
}

impl LfoParams {
    fn new(shape: LfoShape, rate: f32) -> Self {
        Self {
            shape: EnumParam::new("Shape", shape),
            rate: FloatParam::new(
                "Rate",
                rate,
                FloatRange::Skewed { min: 0.02, max: 40.0, factor: FloatRange::skew_factor(-2.0) },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            sync: flag("Tempo Sync", false),
            div: EnumParam::new("Division", LfoDiv::Quarter),
            retrig: flag("Retrigger", true),
        }
    }
}

#[derive(Params)]
pub struct ModSlot {
    #[id = "src"]
    pub src: EnumParam<ModSrc>,
    #[id = "dst"]
    pub dst: EnumParam<ModDst>,
    #[id = "amt"]
    pub amount: FloatParam,
}

impl ModSlot {
    fn new(src: ModSrc, dst: ModDst, amount: f32) -> Self {
        Self {
            src: EnumParam::new("Source", src),
            dst: EnumParam::new("Destination", dst),
            amount: bipolar("Amount", amount),
        }
    }
}

#[derive(Params)]
pub struct DistParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "kind"]
    pub kind: EnumParam<DistKind>,
    #[id = "drv"]
    pub drive: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
}

#[derive(Params)]
pub struct ChorusParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "rate"]
    pub rate: FloatParam,
    #[id = "dep"]
    pub depth: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
}

#[derive(Params)]
pub struct DelayParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "time"]
    pub time: FloatParam,
    #[id = "fb"]
    pub feedback: FloatParam,
    #[id = "pp"]
    pub pingpong: BoolParam,
    #[id = "mix"]
    pub mix: FloatParam,
}

#[derive(Params)]
pub struct ReverbParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "size"]
    pub size: FloatParam,
    #[id = "damp"]
    pub damp: FloatParam,
    #[id = "wid"]
    pub width: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
}

// ---------------------------------------------------------------------------------------------
// Top level
// ---------------------------------------------------------------------------------------------

pub const EDITOR_W: u32 = 1160;
pub const EDITOR_H: u32 = 806;
/// Height of the keyboard strip (and its gap) at the bottom of the standalone window.
pub const KEYBOARD_H: u32 = 70;
pub const MOD_SLOTS: usize = 10;

#[derive(Params)]
pub struct SzParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    /// Index of the last loaded factory preset (only used to label the preset browser).
    #[persist = "preset"]
    pub preset: Arc<std::sync::atomic::AtomicI32>,

    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "vmode"]
    pub voice_mode: EnumParam<VoiceMode>,
    #[id = "poly"]
    pub voices: IntParam,
    #[id = "glide"]
    pub glide: FloatParam,
    #[id = "bend"]
    pub bend_range: IntParam,

    #[nested(id_prefix = "a", group = "Osc A")]
    pub a: OscParams,
    #[nested(id_prefix = "b", group = "Osc B")]
    pub b: OscParams,
    #[nested(id_prefix = "sub", group = "Sub")]
    pub sub: SubParams,
    #[nested(id_prefix = "noi", group = "Noise")]
    pub noise: NoiseParams,
    #[nested(id_prefix = "flt", group = "Filter")]
    pub filter: FilterParams,

    #[nested(array, group = "Envelope")]
    pub env: [EnvParams; 3],
    #[nested(array, group = "LFO")]
    pub lfo: [LfoParams; 4],

    #[id = "mac1"]
    pub macro1: FloatParam,
    #[id = "mac2"]
    pub macro2: FloatParam,
    #[id = "mac3"]
    pub macro3: FloatParam,
    #[id = "mac4"]
    pub macro4: FloatParam,

    #[nested(array, group = "Mod Matrix")]
    pub slots: [ModSlot; MOD_SLOTS],

    #[nested(id_prefix = "dst", group = "Distortion")]
    pub dist: DistParams,
    #[nested(id_prefix = "cho", group = "Chorus")]
    pub chorus: ChorusParams,
    #[nested(id_prefix = "dly", group = "Delay")]
    pub delay: DelayParams,
    #[nested(id_prefix = "rev", group = "Reverb")]
    pub reverb: ReverbParams,
}

impl Default for SzParams {
    fn default() -> Self {
        let mut slots: Vec<ModSlot> = Vec::new();
        slots.push(ModSlot::new(ModSrc::Env2, ModDst::Cutoff, 0.0));
        for _ in 1..MOD_SLOTS {
            slots.push(ModSlot::new(ModSrc::None, ModDst::None, 0.0));
        }
        let slots: [ModSlot; MOD_SLOTS] = match slots.try_into() {
            Ok(a) => a,
            Err(_) => unreachable!(),
        };

        Self {
            // The on-screen keyboard strip only exists in the standalone app.
            editor_state: EguiState::from_size(
                EDITOR_W,
                if plugin_common::midi::is_standalone() { EDITOR_H } else { EDITOR_H - KEYBOARD_H },
            ),
            preset: Arc::new(std::sync::atomic::AtomicI32::new(0)),

            gain: FloatParam::new(
                "Master",
                util::db_to_gain(-4.0),
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
            voice_mode: EnumParam::new("Voice Mode", VoiceMode::Poly),
            voices: IntParam::new("Voices", 8, IntRange::Linear { min: 1, max: 16 }),
            glide: time_ms("Glide", 0.0, 0.0, 2000.0),
            bend_range: IntParam::new("Bend Range", 2, IntRange::Linear { min: 0, max: 24 }),

            a: OscParams::new(true, WtId::Basic, 0.75),
            b: OscParams::new(false, WtId::SawRes, 0.0),
            sub: SubParams {
                on: flag("Enable", false),
                shape: EnumParam::new("Shape", SubShape::Sine),
                octave: IntParam::new("Octave", -1, IntRange::Linear { min: -3, max: 0 }),
                level: level("Level", 0.5),
                to_filter: flag("To Filter", false),
            },
            noise: NoiseParams {
                on: flag("Enable", false),
                level: level("Level", 0.15),
                to_filter: flag("To Filter", true),
            },
            filter: FilterParams {
                on: flag("Enable", true),
                kind: EnumParam::new("Type", FilterKind::Lp24),
                cutoff: FloatParam::new(
                    "Cutoff",
                    8000.0,
                    FloatRange::Skewed { min: 20.0, max: 20_000.0, factor: FloatRange::skew_factor(-2.5) },
                )
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
                .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
                res: pct("Resonance", 0.15),
                drive: pct("Drive", 0.0),
                mix: pct("Mix", 1.0),
                keytrack: pct("Key Track", 0.0),
                env_amt: bipolar("Env Amount", 0.0),
            },

            env: [
                EnvParams::new(1.0, 300.0, 0.8, 150.0),
                EnvParams::new(1.0, 400.0, 0.0, 200.0),
                EnvParams::new(1.0, 600.0, 0.0, 300.0),
            ],
            lfo: [
                LfoParams::new(LfoShape::Sine, 2.0),
                LfoParams::new(LfoShape::Triangle, 0.5),
                LfoParams::new(LfoShape::SampleHold, 4.0),
                LfoParams::new(LfoShape::Square, 1.0),
            ],

            macro1: pct("Macro 1", 0.0),
            macro2: pct("Macro 2", 0.0),
            macro3: pct("Macro 3", 0.0),
            macro4: pct("Macro 4", 0.0),

            slots,

            dist: DistParams {
                on: flag("Enable", false),
                kind: EnumParam::new("Type", DistKind::Soft),
                drive: pct("Drive", 0.3),
                mix: pct("Mix", 1.0),
            },
            chorus: ChorusParams {
                on: flag("Enable", false),
                rate: FloatParam::new(
                    "Rate",
                    0.6,
                    FloatRange::Skewed { min: 0.05, max: 8.0, factor: FloatRange::skew_factor(-1.5) },
                )
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
                depth: pct("Depth", 0.5),
                mix: pct("Mix", 0.4),
            },
            delay: DelayParams {
                on: flag("Enable", false),
                time: time_ms("Time", 350.0, 1.0, 2000.0),
                feedback: pct("Feedback", 0.4),
                pingpong: flag("Ping-Pong", true),
                mix: pct("Mix", 0.3),
            },
            reverb: ReverbParams {
                on: flag("Enable", false),
                size: pct("Size", 0.6),
                damp: pct("Damping", 0.5),
                width: pct("Width", 1.0),
                mix: pct("Mix", 0.25),
            },
        }
    }
}
