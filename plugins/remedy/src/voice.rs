//! Parameter snapshot, polyphonic voices and the voice-allocating engine.
//!
//! The whole voice path (oscillators, warps, filter, master distortion) runs at twice the host
//! sample rate and is brought back down with a linear-phase half-band filter. That keeps the
//! harmonics created by sync, FM, folding and drive from aliasing back into the audible band.

use crate::env::{Adsr, AdsrCoefs, LfoState};
use crate::filter::{FilterBlock, FilterChannel, FilterRamp};
use crate::fx::distort;
use crate::osc::{Osc, OscBlock, SubOsc, Warp};
use crate::params::*;
use crate::util::{lerp, midi_to_hz, Decimator2x, Rng};
use crate::wavetable::{bank, Wavetables};

/// Control-rate block size in host samples. Modulation is evaluated once per block and the values
/// that matter (cutoff, levels, wavetable position) are ramped per sample in between.
pub const MAXB: usize = 16;
/// The same block at the oversampled rate.
const OSB: usize = MAXB * 2;
pub const MAX_VOICES: usize = 16;
const VOICE_SCALE: f32 = 0.6;
/// Latency of the 2x decimator in host samples (rounded up).
pub const LATENCY: u32 = 12;

// ---------------------------------------------------------------------------------------------
// Snapshot: plain-data copy of the parameters, read once per block
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct OscSnap {
    pub on: bool,
    pub table: usize,
    pub pos: f32,
    pub semi: f32,
    pub fine: f32,
    pub level: f32,
    pub pan: f32,
    pub phase: f32,
    pub rand: f32,
    pub unison: usize,
    pub detune: f32,
    pub blend: f32,
    pub spread: f32,
    pub warp: [(WarpMode, f32); 2],
    pub to_filter: bool,
}

#[derive(Clone, Copy)]
pub struct EnvSnap {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

#[derive(Clone, Copy)]
pub struct LfoSnap {
    pub shape: LfoShape,
    pub rate_hz: f32,
    pub retrig: bool,
}

#[derive(Clone, Copy)]
pub struct Snapshot {
    pub osc: [OscSnap; 2],
    pub sub_on: bool,
    pub sub_shape: SubShape,
    pub sub_oct: i32,
    pub sub_level: f32,
    pub sub_to_filter: bool,
    pub noise_on: bool,
    pub noise_level: f32,
    pub noise_to_filter: bool,
    pub filt_on: bool,
    pub filt_kind: FilterKind,
    pub cutoff: f32,
    pub res: f32,
    pub drive: f32,
    pub fmix: f32,
    pub keytrack: f32,
    pub env_amt: f32,
    pub env: [EnvSnap; 3],
    pub lfo: [LfoSnap; 4],
    pub macros: [f32; 4],
    pub slots: [(usize, usize, f32); MOD_SLOTS],
    pub mode: VoiceMode,
    pub nvoices: usize,
    pub glide_ms: f32,
    pub bend_range: f32,
    pub dist_on: bool,
    pub dist_kind: DistKind,
    pub dist_drive: f32,
    pub dist_mix: f32,
}

fn read_osc(o: &OscParams) -> OscSnap {
    OscSnap {
        on: o.on.value(),
        table: o.table.value() as usize,
        pos: o.pos.value(),
        semi: o.semi.value() as f32,
        fine: o.fine.value(),
        level: o.level.value(),
        pan: o.pan.value(),
        phase: o.phase.value(),
        rand: o.rand.value(),
        unison: o.unison.value() as usize,
        detune: o.detune.value(),
        blend: o.blend.value(),
        spread: o.spread.value(),
        warp: [
            (o.w1_mode.value(), o.w1_amt.value()),
            (o.w2_mode.value(), o.w2_amt.value()),
        ],
        to_filter: o.to_filter.value(),
    }
}

fn read_env(e: &EnvParams) -> EnvSnap {
    EnvSnap {
        a: e.attack.value(),
        d: e.decay.value(),
        s: e.sustain.value(),
        r: e.release.value(),
    }
}

fn read_lfo(l: &LfoParams, tempo: f32) -> LfoSnap {
    let rate_hz = if l.sync.value() {
        (tempo / 60.0) / l.div.value().beats()
    } else {
        l.rate.value()
    };
    LfoSnap { shape: l.shape.value(), rate_hz, retrig: l.retrig.value() }
}

impl Snapshot {
    pub fn read(p: &SzParams, tempo: Option<f64>) -> Self {
        let tempo = tempo.unwrap_or(120.0).max(20.0) as f32;
        let mut slots = [(0usize, 0usize, 0.0f32); MOD_SLOTS];
        for (i, s) in p.slots.iter().enumerate() {
            slots[i] = (s.src.value() as usize, s.dst.value() as usize, s.amount.value());
        }
        Self {
            osc: [read_osc(&p.a), read_osc(&p.b)],
            sub_on: p.sub.on.value(),
            sub_shape: p.sub.shape.value(),
            sub_oct: p.sub.octave.value(),
            sub_level: p.sub.level.value(),
            sub_to_filter: p.sub.to_filter.value(),
            noise_on: p.noise.on.value(),
            noise_level: p.noise.level.value(),
            noise_to_filter: p.noise.to_filter.value(),
            filt_on: p.filter.on.value(),
            filt_kind: p.filter.kind.value(),
            cutoff: p.filter.cutoff.value(),
            res: p.filter.res.value(),
            drive: p.filter.drive.value(),
            fmix: p.filter.mix.value(),
            keytrack: p.filter.keytrack.value(),
            env_amt: p.filter.env_amt.value(),
            env: [read_env(&p.env[0]), read_env(&p.env[1]), read_env(&p.env[2])],
            lfo: [
                read_lfo(&p.lfo[0], tempo),
                read_lfo(&p.lfo[1], tempo),
                read_lfo(&p.lfo[2], tempo),
                read_lfo(&p.lfo[3], tempo),
            ],
            macros: [p.macro1.value(), p.macro2.value(), p.macro3.value(), p.macro4.value()],
            slots,
            mode: p.voice_mode.value(),
            nvoices: (p.voices.value() as usize).clamp(1, MAX_VOICES),
            glide_ms: p.glide.value(),
            bend_range: p.bend_range.value() as f32,
            dist_on: p.dist.on.value(),
            dist_kind: p.dist.kind.value(),
            dist_drive: p.dist.drive.value(),
            dist_mix: p.dist.mix.value(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Voice
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
pub struct Globals {
    pub bend_semis: f32,
    pub modwheel: f32,
    pub aftertouch: f32,
}

/// What the newest sounding voice is doing right now, for the editor's moving displays.
#[derive(Clone, Copy, Default)]
pub struct VizFrame {
    pub active: bool,
    /// Modulated wavetable position of oscillator A and B.
    pub pos: [f32; 2],
    /// Modulated filter cutoff in Hz.
    pub cutoff: f32,
    /// (stage, level) of each envelope; see `Adsr::view`.
    pub env: [(u8, f32); 3],
    pub lfo_phase: [f32; 4],
    pub lfo_value: [f32; 4],
}

pub struct BlockCtx<'a> {
    pub snap: &'a Snapshot,
    pub coefs: [AdsrCoefs; 3],
    pub globals: Globals,
    /// Oversampled engine rate.
    pub sr: f32,
    pub bank: &'static Wavetables,
}

pub struct Voice {
    pub active: bool,
    pub gate: bool,
    pub held: bool,
    pub note: u8,
    pub age: u64,
    vel: f32,
    pitch: f32,
    target: f32,
    osc: [Osc; 2],
    sub: SubOsc,
    rng: Rng,
    env: [Adsr; 3],
    lfo: [LfoState; 4],
    filt: [FilterChannel; 2],
    // Where the last block's per-sample ramps ended (NaN = snap to the new value).
    ramp: FilterRamp,
    last_pos: [f32; 2],
    last_level: [f32; 2],
    last_gain: f32,
    viz_cutoff: f32,
    viz_lfo: [f32; 4],
}

impl Voice {
    fn new(seed: u32) -> Self {
        Self {
            active: false,
            gate: false,
            held: false,
            note: 0,
            age: 0,
            vel: 0.0,
            pitch: 60.0,
            target: 60.0,
            osc: [Osc::default(), Osc::default()],
            sub: SubOsc::default(),
            rng: Rng::new(seed),
            env: [Adsr::default(), Adsr::default(), Adsr::default()],
            lfo: [LfoState::default(), LfoState::default(), LfoState::default(), LfoState::default()],
            filt: [FilterChannel::default(), FilterChannel::default()],
            ramp: FilterRamp::default(),
            last_pos: [f32::NAN; 2],
            last_level: [f32::NAN; 2],
            last_gain: f32::NAN,
            viz_cutoff: 1000.0,
            viz_lfo: [0.0; 4],
        }
    }

    fn viz(&self) -> VizFrame {
        let pos = |v: f32| if v.is_finite() { v } else { 0.0 };
        VizFrame {
            active: true,
            pos: [pos(self.last_pos[0]), pos(self.last_pos[1])],
            cutoff: self.viz_cutoff,
            env: [self.env[0].view(), self.env[1].view(), self.env[2].view()],
            lfo_phase: [self.lfo[0].phase, self.lfo[1].phase, self.lfo[2].phase, self.lfo[3].phase],
            lfo_value: self.viz_lfo,
        }
    }

    fn snap_ramps(&mut self) {
        self.ramp = FilterRamp::default();
        self.last_pos = [f32::NAN; 2];
        self.last_level = [f32::NAN; 2];
        self.last_gain = f32::NAN;
    }

    fn kill(&mut self) {
        self.active = false;
        self.gate = false;
        self.held = false;
        self.env.iter_mut().for_each(|e| e.kill());
        self.filt.iter_mut().for_each(|f| f.reset());
    }

    fn start(&mut self, note: u8, vel: f32, age: u64, snap: &Snapshot, global_lfo: &[f32; 4], fresh: bool) {
        let was_active = self.active;
        self.active = true;
        self.gate = true;
        self.held = false;
        self.note = note;
        self.vel = vel;
        self.age = age;
        self.target = note as f32;
        if fresh || !was_active || snap.glide_ms < 0.5 || snap.mode == VoiceMode::Poly {
            self.pitch = self.target;
        }
        if fresh || !was_active {
            self.sub.phase = 0.0;
            for (o, osnap) in snap.osc.iter().enumerate() {
                self.osc[o].reset(osnap.phase, osnap.rand, &mut self.rng);
            }
            self.filt.iter_mut().for_each(|f| f.reset());
            self.snap_ramps();
        }
        for i in 0..4 {
            let phase = if snap.lfo[i].retrig { 0.0 } else { global_lfo[i] };
            self.lfo[i].reset(phase, &mut self.rng);
        }
        self.env.iter_mut().for_each(|e| e.gate_on());
    }

    /// Legato note change: new pitch, envelopes keep running.
    fn retarget(&mut self, note: u8, vel: f32, age: u64) {
        self.note = note;
        self.target = note as f32;
        self.vel = vel;
        self.age = age;
        self.held = false;
    }

    fn release(&mut self) {
        self.gate = false;
        self.held = false;
        self.env.iter_mut().for_each(|e| e.gate_off());
    }

    /// Add `out_l.len()` oversampled samples (at most [`OSB`]) to the outputs.
    fn render(&mut self, ctx: &BlockCtx, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len()).min(OSB);
        if n == 0 {
            return;
        }
        let snap = ctx.snap;
        let sr = ctx.sr;
        let inv_n = 1.0 / n as f32;

        // 1) Envelopes (per sample for the amp envelope, sampled at the block end for the others).
        let mut amp = [0.0f32; OSB];
        let (mut e2, mut e3) = (0.0, 0.0);
        for a in amp.iter_mut().take(n) {
            *a = self.env[0].next(&ctx.coefs[0]);
            e2 = self.env[1].next(&ctx.coefs[1]);
            e3 = self.env[2].next(&ctx.coefs[2]);
        }

        // 2) LFOs
        let mut lfo = [0.0f32; 4];
        for i in 0..4 {
            lfo[i] = self.lfo[i].advance(snap.lfo[i].shape, snap.lfo[i].rate_hz / sr, n, &mut self.rng);
        }

        self.viz_lfo = lfo;

        // 3) Modulation sources and the matrix
        let mut src = [0.0f32; MOD_SRC_COUNT];
        src[ModSrc::Lfo1 as usize] = lfo[0];
        src[ModSrc::Lfo2 as usize] = lfo[1];
        src[ModSrc::Lfo3 as usize] = lfo[2];
        src[ModSrc::Lfo4 as usize] = lfo[3];
        src[ModSrc::Env2 as usize] = e2;
        src[ModSrc::Env3 as usize] = e3;
        src[ModSrc::Velocity as usize] = self.vel;
        src[ModSrc::Key as usize] = self.note as f32 / 127.0;
        src[ModSrc::ModWheel as usize] = ctx.globals.modwheel;
        src[ModSrc::Aftertouch as usize] = ctx.globals.aftertouch;
        for m in 0..4 {
            src[ModSrc::Macro1 as usize + m] = snap.macros[m];
        }
        let mut md = [0.0f32; MOD_DST_COUNT];
        for &(s, d, amt) in snap.slots.iter() {
            if s != 0 && d != 0 && amt != 0.0 {
                md[d] += src[s] * amt;
            }
        }

        // 4) Glide
        if (self.pitch - self.target).abs() > 1e-4 {
            if snap.glide_ms < 0.5 {
                self.pitch = self.target;
            } else {
                let tau = snap.glide_ms * 0.001 * sr / 4.6;
                let k = 1.0 - (-(n as f32) / tau).exp();
                self.pitch += (self.target - self.pitch) * k;
            }
        } else {
            self.pitch = self.target;
        }
        let pitch = self.pitch + ctx.globals.bend_semis;

        // 5) Oscillators. `f*` goes through the filter, `d*` bypasses it.
        let mut fl = [0.0f32; OSB];
        let mut fr = [0.0f32; OSB];
        let mut dl = [0.0f32; OSB];
        let mut dr = [0.0f32; OSB];

        for o in 0..2 {
            let os = &snap.osc[o];
            let pick = |a: ModDst, b: ModDst| md[(if o == 0 { a } else { b }) as usize];
            let level = if os.on { (os.level + pick(ModDst::ALevel, ModDst::BLevel)).clamp(0.0, 1.5) } else { 0.0 };
            let level0 = if self.last_level[o].is_finite() { self.last_level[o] } else { level };
            self.last_level[o] = level;
            let pos = (os.pos + pick(ModDst::APos, ModDst::BPos)).clamp(0.0, 1.0);
            let pos0 = if self.last_pos[o].is_finite() { self.last_pos[o] } else { pos };
            self.last_pos[o] = pos;
            if !os.on || (level <= 0.0001 && level0 <= 0.0001) {
                continue;
            }
            let semis = pitch + os.semi + os.fine / 100.0 + pick(ModDst::APitch, ModDst::BPitch) * 24.0;
            let warp_amt = |i: usize| {
                let m = if i == 0 {
                    pick(ModDst::AWarp1, ModDst::BWarp1)
                } else {
                    pick(ModDst::AWarp2, ModDst::BWarp2)
                };
                os.warp[i].1 + m
            };
            let block = OscBlock {
                freq: midi_to_hz(semis),
                pos0,
                pos1: pos,
                unison: os.unison,
                detune: (os.detune + pick(ModDst::ADetune, ModDst::BDetune)).clamp(0.0, 1.0),
                blend: os.blend,
                spread: os.spread,
                warp: [
                    Warp::new(os.warp[0].0, warp_amt(0)),
                    Warp::new(os.warp[1].0, warp_amt(1)),
                ],
                pan: os.pan + pick(ModDst::APan, ModDst::BPan),
                sample_rate: sr,
            };
            let table = &ctx.bank.tables[os.table.min(ctx.bank.tables.len() - 1)];
            let mut tl = [0.0f32; OSB];
            let mut tr = [0.0f32; OSB];
            self.osc[o].render(table, &block, &mut tl[..n], &mut tr[..n]);
            let dlevel = (level - level0) * inv_n;
            let (bl, br) = if os.to_filter && snap.filt_on { (&mut fl, &mut fr) } else { (&mut dl, &mut dr) };
            for s in 0..n {
                let g = level0 + dlevel * (s + 1) as f32;
                bl[s] += tl[s] * g;
                br[s] += tr[s] * g;
            }
        }

        // Sub oscillator and noise (mono sources)
        if snap.sub_on {
            let level = (snap.sub_level + md[ModDst::SubLevel as usize]).clamp(0.0, 1.5);
            let hz = midi_to_hz(pitch) * (snap.sub_oct as f32).exp2();
            let inc = (hz / sr).min(0.45);
            let to_f = snap.sub_to_filter && snap.filt_on;
            for s in 0..n {
                let y = self.sub.next(snap.sub_shape, inc) * level * 0.7;
                if to_f {
                    fl[s] += y;
                    fr[s] += y;
                } else {
                    dl[s] += y;
                    dr[s] += y;
                }
            }
        }
        if snap.noise_on {
            let level = (snap.noise_level + md[ModDst::NoiseLevel as usize]).clamp(0.0, 1.5) * 0.5;
            let to_f = snap.noise_to_filter && snap.filt_on;
            for s in 0..n {
                let (a, b) = (self.rng.next_bipolar() * level, self.rng.next_bipolar() * level);
                if to_f {
                    fl[s] += a;
                    fr[s] += b;
                } else {
                    dl[s] += a;
                    dr[s] += b;
                }
            }
        }

        // 6) Filter and amplifier
        let vgain = VOICE_SCALE * lerp(0.2, 1.0, self.vel) * (1.0 + md[ModDst::Amp as usize]).clamp(0.0, 2.0);
        let vgain0 = if self.last_gain.is_finite() { self.last_gain } else { vgain };
        self.last_gain = vgain;
        let dgain = (vgain - vgain0) * inv_n;
        if snap.filt_on {
            let octaves = snap.env_amt * e2 * 7.0
                + snap.keytrack * (self.pitch - 60.0) / 12.0
                + md[ModDst::Cutoff as usize] * 5.0;
            let cutoff = (snap.cutoff * octaves.clamp(-12.0, 12.0).exp2()).clamp(20.0, 21_000.0);
            self.viz_cutoff = cutoff;
            let res = (snap.res + md[ModDst::Resonance as usize]).clamp(0.0, 1.0);
            let drive = (snap.drive + md[ModDst::FilterDrive as usize]).clamp(0.0, 1.0);
            let fb = FilterBlock::new(snap.filt_kind, cutoff, res, drive, snap.fmix, sr, n, &mut self.ramp);
            for s in 0..n {
                let yl = self.filt[0].process(fl[s], &fb, s + 1) + dl[s];
                let yr = self.filt[1].process(fr[s], &fb, s + 1) + dr[s];
                let g = amp[s] * (vgain0 + dgain * (s + 1) as f32);
                out_l[s] += yl * g;
                out_r[s] += yr * g;
            }
        } else {
            for s in 0..n {
                let g = amp[s] * (vgain0 + dgain * (s + 1) as f32);
                out_l[s] += (fl[s] + dl[s]) * g;
                out_r[s] += (fr[s] + dr[s]) * g;
            }
        }

        if self.env[0].is_idle() {
            self.active = false;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------------------------

pub struct Engine {
    voices: Vec<Voice>,
    /// Oversampled rate (2x the host rate).
    sr: f32,
    age: u64,
    pub(crate) snap: Snapshot,
    bank: &'static Wavetables,
    note_stack: Vec<(u8, f32)>,
    lfo_phase: [f32; 4],
    bend_norm: f32,
    modwheel: f32,
    aftertouch: f32,
    sustain: bool,
    dec: [Decimator2x; 2],
    /// Snapshot of the newest sounding voice, refreshed every block.
    pub viz: VizFrame,
}

impl Engine {
    /// `host_rate` is the host sample rate; the engine runs internally at twice that.
    pub fn new(host_rate: f32, params: &SzParams) -> Self {
        Self {
            voices: (0..MAX_VOICES).map(|i| Voice::new(0x9E37_79B9u32.wrapping_mul(i as u32 + 1))).collect(),
            sr: host_rate * 2.0,
            age: 0,
            snap: Snapshot::read(params, None),
            bank: bank(),
            note_stack: Vec::with_capacity(32),
            lfo_phase: [0.0; 4],
            bend_norm: 0.0,
            modwheel: 0.0,
            aftertouch: 0.0,
            sustain: false,
            dec: [Decimator2x::new(), Decimator2x::new()],
            viz: VizFrame::default(),
        }
    }

    pub fn reset(&mut self) {
        self.voices.iter_mut().for_each(|v| v.kill());
        self.note_stack.clear();
        self.sustain = false;
        self.dec.iter_mut().for_each(|d| d.reset());
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    fn alloc(&self) -> usize {
        let nv = self.snap.nvoices.min(MAX_VOICES);
        if let Some(i) = (0..nv).find(|&i| !self.voices[i].active) {
            return i;
        }
        // Prefer stealing the oldest releasing voice, then the oldest voice overall.
        let oldest = |only_released: bool| {
            (0..nv)
                .filter(|&i| !only_released || !self.voices[i].gate)
                .min_by_key(|&i| self.voices[i].age)
        };
        oldest(true).or_else(|| oldest(false)).unwrap_or(0)
    }

    pub fn note_on(&mut self, note: u8, vel: f32) {
        self.age += 1;
        let age = self.age;
        match self.snap.mode {
            VoiceMode::Poly => {
                let i = self.alloc();
                let lp = self.lfo_phase;
                // A stolen voice keeps its oscillator phases, filter state and envelope level, so
                // the takeover is a quick pitch change instead of a click.
                let fresh = !self.voices[i].active;
                self.voices[i].start(note, vel, age, &self.snap, &lp, fresh);
            }
            VoiceMode::Mono | VoiceMode::Legato => {
                self.note_stack.retain(|&(n, _)| n != note);
                if self.note_stack.len() < 32 {
                    self.note_stack.push((note, vel));
                }
                let lp = self.lfo_phase;
                let legato = self.snap.mode == VoiceMode::Legato && self.voices[0].active && self.voices[0].gate;
                if legato {
                    self.voices[0].retarget(note, vel, age);
                } else {
                    let fresh = !self.voices[0].active;
                    self.voices[0].start(note, vel, age, &self.snap, &lp, fresh);
                }
            }
        }
    }

    pub fn note_off(&mut self, note: u8) {
        match self.snap.mode {
            VoiceMode::Poly => {
                let sustain = self.sustain;
                for v in self.voices.iter_mut().filter(|v| v.active && v.gate && v.note == note) {
                    if sustain {
                        v.held = true;
                    } else {
                        v.release();
                    }
                }
            }
            VoiceMode::Mono | VoiceMode::Legato => {
                self.note_stack.retain(|&(n, _)| n != note);
                if !(self.voices[0].active && self.voices[0].gate && self.voices[0].note == note) {
                    return;
                }
                if let Some(&(prev, vel)) = self.note_stack.last() {
                    self.age += 1;
                    let age = self.age;
                    if self.snap.mode == VoiceMode::Legato {
                        self.voices[0].retarget(prev, vel, age);
                    } else {
                        let lp = self.lfo_phase;
                        self.voices[0].start(prev, vel, age, &self.snap, &lp, false);
                    }
                } else if self.sustain {
                    self.voices[0].held = true;
                } else {
                    self.voices[0].release();
                }
            }
        }
    }

    pub fn pitch_bend(&mut self, normalized: f32) {
        self.bend_norm = normalized * 2.0 - 1.0;
    }

    pub fn control_change(&mut self, cc: u8, value: f32) {
        match cc {
            1 => self.modwheel = value,
            64 => {
                let on = value >= 0.5;
                if self.sustain && !on {
                    for v in self.voices.iter_mut().filter(|v| v.active && v.held) {
                        v.release();
                    }
                }
                self.sustain = on;
            }
            120 | 123 => {
                if cc == 120 {
                    self.reset();
                } else {
                    for v in self.voices.iter_mut().filter(|v| v.active) {
                        v.release();
                    }
                    self.note_stack.clear();
                }
            }
            _ => {}
        }
    }

    pub fn channel_pressure(&mut self, value: f32) {
        self.aftertouch = value;
    }

    /// Render `l.len()` host samples (at most [`MAXB`]); the buffers are overwritten.
    pub fn render(&mut self, params: &SzParams, tempo: Option<f64>, l: &mut [f32], r: &mut [f32]) {
        self.snap = Snapshot::read(params, tempo);
        self.render_current(l, r);
    }

    /// Render using whatever snapshot the engine currently holds.
    pub(crate) fn render_current(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len()).min(MAXB);
        let n2 = n * 2;
        let sr = self.sr;

        for i in 0..4 {
            let p = self.lfo_phase[i] + self.snap.lfo[i].rate_hz / sr * n2 as f32;
            self.lfo_phase[i] = p - p.floor();
        }

        let e = &self.snap.env;
        let coefs = [
            AdsrCoefs::new(e[0].a, e[0].d, e[0].s, e[0].r, sr),
            AdsrCoefs::new(e[1].a, e[1].d, e[1].s, e[1].r, sr),
            AdsrCoefs::new(e[2].a, e[2].d, e[2].s, e[2].r, sr),
        ];
        let globals = Globals {
            bend_semis: self.bend_norm * self.snap.bend_range,
            modwheel: self.modwheel,
            aftertouch: self.aftertouch,
        };
        let mut bl = [0.0f32; OSB];
        let mut br = [0.0f32; OSB];
        let ctx = BlockCtx { snap: &self.snap, coefs, globals, sr, bank: self.bank };
        for v in self.voices.iter_mut().filter(|v| v.active) {
            v.render(&ctx, &mut bl[..n2], &mut br[..n2]);
        }

        self.viz = self
            .voices
            .iter()
            .filter(|v| v.active)
            .max_by_key(|v| v.age)
            .map(|v| v.viz())
            .unwrap_or_default();

        // Master distortion, still oversampled so its harmonics do not alias.
        if self.snap.dist_on {
            let (kind, drive, mix) = (self.snap.dist_kind, self.snap.dist_drive, self.snap.dist_mix);
            for s in 0..n2 {
                bl[s] = lerp(bl[s], distort(kind, bl[s], drive), mix);
                br[s] = lerp(br[s], distort(kind, br[s], drive), mix);
            }
        }

        for s in 0..n {
            l[s] = self.dec[0].process(bl[2 * s], bl[2 * s + 1]);
            r[s] = self.dec[1].process(br[2 * s], br[2 * s + 1]);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Render with the engine's current snapshot (tests edit `engine.snap` directly).
    fn render_seconds(engine: &mut Engine, secs: f32) -> (f32, bool) {
        let total = (secs * 48_000.0) as usize;
        let mut peak = 0.0f32;
        let mut finite = true;
        let mut done = 0;
        while done < total {
            let mut l = [0.0f32; MAXB];
            let mut r = [0.0f32; MAXB];
            engine.render_current(&mut l, &mut r);
            for v in l.iter().chain(r.iter()) {
                finite &= v.is_finite();
                peak = peak.max(v.abs());
            }
            done += MAXB;
        }
        (peak, finite)
    }

    /// Render `secs` of the left channel with the engine's current snapshot.
    pub(crate) fn render_current(engine: &mut Engine, secs: f32) -> Vec<f32> {
        let total = (secs * 48_000.0) as usize;
        let mut out = Vec::with_capacity(total + MAXB);
        while out.len() < total {
            let mut l = [0.0f32; MAXB];
            let mut r = [0.0f32; MAXB];
            engine.render_current(&mut l, &mut r);
            out.extend_from_slice(&l);
        }
        out
    }

    #[test]
    fn plays_a_note_and_goes_silent() {
        let params = SzParams::default();
        let mut e = Engine::new(48_000.0, &params);
        e.note_on(60, 0.8);
        let (peak, finite) = render_seconds(&mut e, 0.5);
        assert!(finite);
        assert!(peak > 0.05, "silent, peak {peak}");
        assert!(peak < 1.5, "too loud, peak {peak}");
        assert_eq!(e.active_voices(), 1);
        e.note_off(60);
        let (_, finite) = render_seconds(&mut e, 1.5);
        assert!(finite);
        assert_eq!(e.active_voices(), 0);
    }

    #[test]
    fn middle_c_has_the_right_pitch() {
        let params = SzParams::default();
        let mut e = Engine::new(48_000.0, &params);
        e.snap.filt_on = false;
        e.snap.osc[0].pos = 0.0; // pure sine
        e.note_on(60, 1.0);
        render_seconds(&mut e, 0.3); // get past the attack
        let mut crossings = 0;
        let mut last = 0.0f32;
        let total = 48_000;
        let mut done = 0;
        while done < total {
            let mut l = [0.0f32; MAXB];
            let mut r = [0.0f32; MAXB];
            e.render_current(&mut l, &mut r);
            for &v in l.iter() {
                if last <= 0.0 && v > 0.0 {
                    crossings += 1;
                }
                last = v;
            }
            done += MAXB;
        }
        // Middle C = 261.63 Hz; table position 0 of Basic Shapes is a pure sine.
        assert!((crossings as i32 - 262).abs() <= 2, "counted {crossings} cycles in 1 s");
    }

    #[test]
    fn every_filter_and_warp_mode_is_stable() {
        use nih_plug::prelude::Enum;
        let params = SzParams::default();
        for kind in 0..8 {
            for warp in 0..11 {
                let mut e = Engine::new(48_000.0, &params);
                e.snap.filt_kind = FilterKind::from_index(kind);
                e.snap.res = 1.0;
                e.snap.drive = 1.0;
                e.snap.cutoff = 300.0;
                for o in 0..2 {
                    e.snap.osc[o].on = true;
                    e.snap.osc[o].level = 0.7;
                    e.snap.osc[o].unison = 16;
                    e.snap.osc[o].warp = [
                        (WarpMode::from_index(warp), 1.0),
                        (WarpMode::from_index((warp + 3) % 11), 0.8),
                    ];
                }
                e.note_on(24, 1.0);
                e.note_on(96, 1.0);
                let (peak, finite) = render_seconds(&mut e, 0.25);
                assert!(finite, "filter {kind} warp {warp} produced NaN/inf");
                assert!(peak < 20.0, "filter {kind} warp {warp} peaked at {peak}");
            }
        }
    }

    #[test]
    fn polyphony_and_stealing_stay_finite() {
        let params = SzParams::default();
        let mut e = Engine::new(48_000.0, &params);
        for i in 0..40 {
            e.note_on(36 + i, 0.7);
        }
        let (peak, finite) = render_seconds(&mut e, 0.3);
        assert!(finite);
        assert!(peak.is_finite());
        assert!(e.active_voices() <= 8);
    }

    /// A bright, hard-synced note high up the keyboard is the worst case for aliasing. Everything
    /// that is not a harmonic of the note is aliasing; it has to stay far below the signal.
    #[test]
    fn sync_warp_aliasing_stays_low() {
        let params = SzParams::default();
        let mut e = Engine::new(48_000.0, &params);
        e.snap.filt_on = false;
        e.snap.osc[0].pos = 0.5; // saw
        e.snap.osc[0].warp = [(WarpMode::Sync, 0.6), (WarpMode::Off, 0.0)];
        e.snap.osc[0].rand = 0.0;
        e.snap.env[0] = EnvSnap { a: 1.0, d: 100.0, s: 1.0, r: 100.0 };
        let note = 84u8; // C6, 1046.5 Hz
        e.note_on(note, 1.0);
        render_seconds(&mut e, 0.2);
        let n = 48_000usize;
        let mut x = Vec::with_capacity(n + MAXB);
        while x.len() < n {
            let mut l = [0.0f32; MAXB];
            let mut r = [0.0f32; MAXB];
            e.render_current(&mut l, &mut r);
            x.extend_from_slice(&l);
        }
        x.truncate(n);
        // One-second window => 1 Hz bins. Measure energy at harmonics vs. everywhere else < 16 kHz.
        let f0 = midi_to_hz(note as f32) as f64;
        let power = |hz: f64| {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in x.iter().enumerate() {
                let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
                let ph = std::f64::consts::TAU * hz * i as f64 / 48_000.0;
                re += *v as f64 * w * ph.cos();
                im += *v as f64 * w * ph.sin();
            }
            re * re + im * im
        };
        let mut harmonic = 0.0;
        let mut k = 1.0;
        while k * f0 < 16_000.0 {
            harmonic += power(k * f0);
            k += 1.0;
        }
        // Probe the gaps between harmonics, where only aliases can live.
        let mut alias: f64 = 0.0;
        let mut k = 0.0;
        while (k + 0.5) * f0 < 16_000.0 {
            for frac in [0.23, 0.5, 0.77] {
                alias = alias.max(power((k + frac) * f0));
            }
            k += 1.0;
        }
        let db = 10.0 * (alias / harmonic).log10();
        assert!(db < -50.0, "worst alias is only {db:.1} dB below the signal");
    }
}
