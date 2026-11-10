//! The monophonic wobble voice: parameter snapshot, LFO / pattern logic and the signal path.
//!
//! Signal flow (everything runs at 4x the host rate, so the heavy distortion does not alias):
//!
//! ```text
//! Osc1 (unison) + Osc2 (+FM into Osc1) + Noise [+ Sub] -> pre-drive -> Filter -> [+ clean Sub]
//!   -> Distortion A -> Formant -> Distortion B -> Low/High cut -> OTT -> Amp (env, gate, LFO)
//!   -> mono below 150 Hz -> 4x decimate
//! ```

use crate::dist::shape;
use crate::filter::{Coefs, FilterCh, FormantCh, FormantCoefs, ToneCoefs, ToneCh};
use crate::lfo::Lfo;
use crate::osc::{Osc2, Sub, Unison};
use crate::ott::Ott;
use crate::params::*;
use plugin_common::dsp::{fast_tanh, lerp, midi_to_hz, Adsr, AdsrCoefs, Decimator2x, Rng};

/// Control block size in host samples.
pub const MAXB: usize = 32;
/// Oversampling factor of the whole voice.
const OS: usize = 4;
const OUT_SCALE: f32 = 0.5;
/// Latency of the two decimator stages in host samples (rounded up).
pub const LATENCY: u32 = 18;

// ---------------------------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct LfoSnap {
    pub shape: LfoShape,
    pub sync: bool,
    pub beats: f32,
    pub hz: f32,
    pub trig: LfoTrig,
    pub phase: f32,
    pub smooth: f32,
    pub bipolar: bool,
    pub cut: f32,
    pub res: f32,
    pub drive: f32,
    pub formant: f32,
    pub fm: f32,
    pub pwm: f32,
    pub pitch: f32,
    pub amp: f32,
}

#[derive(Clone, Copy)]
pub struct StepSnap {
    pub on: bool,
    pub beats: f32,
    pub depth: f32,
    pub semi: f32,
}

#[derive(Clone, Copy)]
pub struct DistSnap {
    pub kind: DistKind,
    pub drive: f32,
    pub mix: f32,
}

#[derive(Clone, Copy)]
pub struct EnvSnap {
    pub a: f32,
    pub d: f32,
    pub s: f32,
    pub r: f32,
}

#[derive(Clone, Copy)]
pub struct Snap {
    pub wave1: Wave1,
    pub pwm: f32,
    pub voices: usize,
    pub detune: f32,
    pub spread: f32,
    pub level1: f32,
    pub rand: f32,
    pub wave2: Wave2,
    pub semi2: f32,
    pub fine2: f32,
    pub level2: f32,
    pub fm: f32,
    pub sub_wave: SubWave,
    pub sub_oct: i32,
    pub sub_level: f32,
    pub sub_to_filter: bool,
    pub noise: f32,
    pub filt_kind: FilterKind,
    pub cutoff: f32,
    pub res: f32,
    pub filt_drive: f32,
    pub keytrack: f32,
    pub env_amt: f32,
    pub fmt_mix: f32,
    pub fmt_morph: f32,
    pub fmt_q: f32,
    pub dist_a: DistSnap,
    pub dist_b: DistSnap,
    pub low_cut: f32,
    pub high_cut: f32,
    pub ott: f32,
    pub amp_env: EnvSnap,
    pub filt_env: EnvSnap,
    pub lfo: [LfoSnap; 2],
    pub pat_on: bool,
    pub pat_len: usize,
    pub step_beats: f32,
    pub steps: [StepSnap; STEPS],
    pub glide_ms: f32,
    pub legato: bool,
    pub bend_range: f32,
    pub wheel_amt: f32,
    pub vel_sens: f32,
}

fn read_env(e: &EnvParams) -> EnvSnap {
    EnvSnap { a: e.attack.value(), d: e.decay.value(), s: e.sustain.value(), r: e.release.value() }
}

fn read_dist(d: &DistParams) -> DistSnap {
    DistSnap { kind: d.kind.value(), drive: d.drive.value(), mix: d.mix.value() }
}

fn read_lfo(l: &LfoParams) -> LfoSnap {
    LfoSnap {
        shape: l.shape.value(),
        sync: l.sync.value(),
        beats: l.div.value().beats(),
        hz: l.hz.value(),
        trig: l.trig.value(),
        phase: l.phase.value(),
        smooth: l.smooth.value(),
        bipolar: l.bipolar.value(),
        cut: l.cutoff.value(),
        res: l.res.value(),
        drive: l.drive.value(),
        formant: l.formant.value(),
        fm: l.fm.value(),
        pwm: l.pwm.value(),
        pitch: l.pitch.value(),
        amp: l.amp.value(),
    }
}

impl Snap {
    pub fn read(p: &WubParams) -> Self {
        let mut steps = [StepSnap { on: true, beats: 0.5, depth: 1.0, semi: 0.0 }; STEPS];
        for (i, s) in p.pattern.steps.iter().enumerate() {
            steps[i] = StepSnap {
                on: s.on.value(),
                beats: s.rate.value().beats(),
                depth: s.depth.value(),
                semi: s.semi.value() as f32,
            };
        }
        Self {
            wave1: p.osc1.wave.value(),
            pwm: p.osc1.pwm.value(),
            voices: p.osc1.voices.value() as usize,
            detune: p.osc1.detune.value(),
            spread: p.osc1.spread.value(),
            level1: p.osc1.level.value(),
            rand: p.osc1.rand.value(),
            wave2: p.osc2.wave.value(),
            semi2: p.osc2.semi.value() as f32,
            fine2: p.osc2.fine.value(),
            level2: p.osc2.level.value(),
            fm: p.osc2.fm.value(),
            sub_wave: p.sub.wave.value(),
            sub_oct: p.sub.octave.value(),
            sub_level: p.sub.level.value(),
            sub_to_filter: p.sub.to_filter.value(),
            noise: p.noise.value(),
            filt_kind: p.filter.kind.value(),
            cutoff: p.filter.cutoff.value(),
            res: p.filter.res.value(),
            filt_drive: p.filter.drive.value(),
            keytrack: p.filter.keytrack.value(),
            env_amt: p.filter.env_amt.value(),
            fmt_mix: p.formant.mix.value(),
            fmt_morph: p.formant.morph.value(),
            fmt_q: p.formant.q.value(),
            dist_a: read_dist(&p.dist_a),
            dist_b: read_dist(&p.dist_b),
            low_cut: p.tone.low_cut.value(),
            high_cut: p.tone.high_cut.value(),
            ott: p.tone.ott.value(),
            amp_env: read_env(&p.amp_env),
            filt_env: read_env(&p.filt_env),
            lfo: [read_lfo(&p.lfo1), read_lfo(&p.lfo2)],
            pat_on: p.pattern.on.value(),
            pat_len: (p.pattern.len.value() as usize).clamp(1, STEPS),
            step_beats: p.pattern.step.value().beats(),
            steps,
            glide_ms: p.glide.value(),
            legato: p.legato.value(),
            bend_range: p.bend_range.value() as f32,
            wheel_amt: p.wheel_amt.value(),
            vel_sens: p.vel_sens.value(),
        }
    }
}

/// Host timing for one block.
#[derive(Clone, Copy)]
pub struct Transport {
    pub tempo: f32,
    pub playing: bool,
    /// Song position in quarter notes at the start of the block, if the host provides it.
    pub beat: Option<f64>,
}

impl Default for Transport {
    fn default() -> Self {
        Self { tempo: 120.0, playing: false, beat: None }
    }
}

// ---------------------------------------------------------------------------------------------
// Block-rate modulation values, interpolated per sample
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Mods {
    cut_oct: f32,
    res: f32,
    drive: f32,
    formant: f32,
    fm: f32,
    pwm: f32,
    pitch: f32,
    amp: f32,
}

impl Default for Mods {
    fn default() -> Self {
        Self { cut_oct: 0.0, res: 0.0, drive: 0.0, formant: 0.0, fm: 0.0, pwm: 0.0, pitch: 0.0, amp: 1.0 }
    }
}

impl Mods {
    #[inline]
    fn lerp(a: &Mods, b: &Mods, t: f32) -> Mods {
        Mods {
            cut_oct: lerp(a.cut_oct, b.cut_oct, t),
            res: lerp(a.res, b.res, t),
            drive: lerp(a.drive, b.drive, t),
            formant: lerp(a.formant, b.formant, t),
            fm: lerp(a.fm, b.fm, t),
            pwm: lerp(a.pwm, b.pwm, t),
            pitch: lerp(a.pitch, b.pitch, t),
            amp: lerp(a.amp, b.amp, t),
        }
    }
}

struct Ctx<'a> {
    snap: &'a Snap,
    prev: Mods,
    now: Mods,
    amp_c: AdsrCoefs,
    fil_c: AdsrCoefs,
    inc2: f32,
    inc_sub: f32,
    base_cut: f32,
    tone: ToneCoefs,
    out_gain: f32,
    gate_target: f32,
    gate_k: f32,
    n_i: f32,
    isr: f32,
    pre_gain: f32,
    pre_norm: f32,
    side_k: f32,
}

// ---------------------------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------------------------

pub struct Engine {
    sr: f32,
    isr: f32,
    stack: Vec<(u8, f32)>,
    gate: bool,
    sustain: bool,
    held_off: bool,
    note: u8,
    vel: f32,
    pitch: f32,
    target: f32,
    pitch_off: f32,
    bend: f32,
    wheel: f32,
    fresh: bool,
    uni: Unison,
    osc2: Osc2,
    sub: Sub,
    rng: Rng,
    amp_env: Adsr,
    filt_env: Adsr,
    lfo: [Lfo; 2],
    note_beats: f64,
    gate_gain: f32,
    prev: Mods,
    flt: [FilterCh; 2],
    fmt: [FormantCh; 2],
    tone: [ToneCh; 2],
    ott: Ott,
    fmt_coefs: FormantCoefs,
    side_lp: f32,
    dec_a: [Decimator2x; 2],
    dec_b: [Decimator2x; 2],
    /// Latest LFO outputs and pattern step for the editor (step is -1 when the pattern is off).
    pub lfo_view: [f32; 2],
    pub phase_view: [f32; 2],
    pub step_view: i32,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sr: sample_rate,
            isr: sample_rate * OS as f32,
            stack: Vec::with_capacity(32),
            gate: false,
            sustain: false,
            held_off: false,
            note: 36,
            vel: 1.0,
            pitch: 36.0,
            target: 36.0,
            pitch_off: 0.0,
            bend: 0.0,
            wheel: 0.0,
            fresh: true,
            uni: Unison::default(),
            osc2: Osc2::default(),
            sub: Sub::default(),
            rng: Rng::new(0x1234_5678),
            amp_env: Adsr::default(),
            filt_env: Adsr::default(),
            lfo: [Lfo::default(), Lfo::default()],
            note_beats: 0.0,
            gate_gain: 1.0,
            prev: Mods::default(),
            flt: [FilterCh::default(), FilterCh::default()],
            fmt: [FormantCh::default(), FormantCh::default()],
            tone: [ToneCh::default(), ToneCh::default()],
            ott: Ott::new(sample_rate * OS as f32),
            fmt_coefs: FormantCoefs::new(0.0, 0.5, sample_rate * OS as f32),
            side_lp: 0.0,
            dec_a: [Decimator2x::new(), Decimator2x::new()],
            dec_b: [Decimator2x::new(), Decimator2x::new()],
            lfo_view: [0.0; 2],
            phase_view: [0.0; 2],
            step_view: -1,
        }
    }

    pub fn reset(&mut self) {
        self.stack.clear();
        self.gate = false;
        self.held_off = false;
        self.amp_env.kill();
        self.filt_env.kill();
        self.flt.iter_mut().for_each(|f| f.reset());
        self.fmt.iter_mut().for_each(|f| f.reset());
        self.tone.iter_mut().for_each(|t| *t = ToneCh::default());
        self.ott.reset();
        self.side_lp = 0.0;
        self.dec_a.iter_mut().for_each(|d| d.reset());
        self.dec_b.iter_mut().for_each(|d| d.reset());
        self.fresh = true;
    }

    pub fn is_sounding(&self) -> bool {
        !self.amp_env.is_idle()
    }

    // ----- MIDI -----------------------------------------------------------------------------

    pub fn note_on(&mut self, note: u8, vel: f32, snap: &Snap) {
        self.stack.retain(|&(n, _)| n != note);
        if self.stack.len() < 32 {
            self.stack.push((note, vel));
        }
        let was_sounding = self.is_sounding();
        let legato = snap.legato && self.gate && was_sounding;
        self.note = note;
        self.vel = vel;
        self.target = note as f32;
        self.gate = true;
        self.held_off = false;
        if legato {
            return; // slide to the new pitch, keep envelopes and LFOs running
        }
        if !was_sounding || snap.glide_ms < 0.5 {
            self.pitch = self.target;
        }
        if !was_sounding {
            self.uni.reset(snap.rand, &mut self.rng);
            self.osc2.reset();
            self.sub.reset();
            self.flt.iter_mut().for_each(|f| f.reset());
            self.fmt.iter_mut().for_each(|f| f.reset());
            self.ott.reset();
            self.fresh = true;
        }
        self.amp_env.gate_on();
        self.filt_env.gate_on();
        for k in 0..2 {
            if snap.lfo[k].trig == LfoTrig::Note {
                self.lfo[k].restart(&mut self.rng);
            }
        }
        self.note_beats = 0.0;
    }

    pub fn note_off(&mut self, note: u8) {
        self.stack.retain(|&(n, _)| n != note);
        if note != self.note || !self.gate {
            return;
        }
        if let Some(&(prev, vel)) = self.stack.last() {
            self.note = prev;
            self.target = prev as f32;
            self.vel = vel;
        } else if self.sustain {
            self.held_off = true;
        } else {
            self.release();
        }
    }

    fn release(&mut self) {
        self.gate = false;
        self.held_off = false;
        self.amp_env.gate_off();
        self.filt_env.gate_off();
    }

    pub fn pitch_bend(&mut self, normalized: f32) {
        self.bend = normalized * 2.0 - 1.0;
    }

    pub fn control_change(&mut self, cc: u8, value: f32) {
        match cc {
            1 => self.wheel = value,
            64 => {
                let on = value >= 0.5;
                if self.sustain && !on && self.held_off {
                    self.release();
                }
                self.sustain = on;
            }
            120 | 123 => {
                self.stack.clear();
                if self.gate {
                    self.release();
                }
            }
            _ => {}
        }
    }

    // ----- Rendering ------------------------------------------------------------------------

    /// Render up to [`MAXB`] host samples into `out_l` / `out_r` (overwritten).
    pub fn render(&mut self, snap: &Snap, tr: &Transport, out_l: &mut [f32], out_r: &mut [f32]) {
        let n = out_l.len().min(out_r.len()).min(MAXB);
        if n == 0 {
            return;
        }
        if !self.is_sounding() {
            out_l[..n].fill(0.0);
            out_r[..n].fill(0.0);
            self.lfo_view = [0.0; 2];
            self.step_view = -1;
            return;
        }

        let sr = self.sr;
        let isr = self.isr;
        let block_secs = n as f32 / sr;
        let bps = tr.tempo.clamp(20.0, 400.0) / 60.0;
        let dt_beats = block_secs * bps;
        let grid_ok = tr.playing && tr.beat.is_some();

        // --- Pattern step -------------------------------------------------------------------
        let mut step: Option<StepSnap> = None;
        self.step_view = -1;
        if snap.pat_on {
            let step_beats = snap.step_beats.max(0.01) as f64;
            let use_grid = snap.lfo[0].trig == LfoTrig::Grid && grid_ok;
            let pos = if use_grid { tr.beat.unwrap_or(0.0) } else { self.note_beats };
            let idx = ((pos / step_beats).floor().max(0.0) as usize) % snap.pat_len;
            step = Some(snap.steps[idx]);
            self.step_view = idx as i32;
        }

        // --- LFOs ---------------------------------------------------------------------------
        let mut lfo_vals = [0.0f32; 2];
        for k in 0..2 {
            let l = &snap.lfo[k];
            let pattern_rate = if k == 0 { step.map(|s| s.beats) } else { None };
            let sync_active = pattern_rate.is_some() || l.sync;
            let beats_per_cycle = pattern_rate.unwrap_or(l.beats).max(0.01);
            let cycles = if sync_active { dt_beats / beats_per_cycle } else { l.hz * block_secs };
            if l.trig == LfoTrig::Grid && grid_ok && sync_active && pattern_rate.is_none() {
                let ph = (tr.beat.unwrap_or(0.0) + dt_beats as f64) / beats_per_cycle as f64;
                self.lfo[k].set_phase((ph - ph.floor()) as f32, &mut self.rng);
            } else {
                self.lfo[k].advance(cycles, &mut self.rng);
            }
            lfo_vals[k] = self.lfo[k].output(l.shape, l.phase, l.smooth, l.bipolar, block_secs);
        }
        self.lfo_view = lfo_vals;
        self.phase_view = [self.lfo[0].phase, self.lfo[1].phase];

        // LFO 1 depth: pattern step depth and the mod wheel scale everything LFO 1 does.
        let wheel_scale = lerp(1.0, self.wheel, snap.wheel_amt);
        let depth1 = step.map(|s| s.depth).unwrap_or(1.0) * wheel_scale;

        let mut mods = Mods::default();
        for k in 0..2 {
            let l = &snap.lfo[k];
            let v = lfo_vals[k] * if k == 0 { depth1 } else { 1.0 };
            mods.cut_oct += v * l.cut * 6.0;
            mods.res += v * l.res * 0.5;
            mods.drive += v * l.drive * 0.6;
            mods.formant += v * l.formant * 0.5;
            mods.fm += v * l.fm * 0.5;
            mods.pwm += v * l.pwm * 0.4;
            mods.pitch += v * l.pitch * 12.0;
            let v01 = if l.amp >= 0.0 { v.clamp(0.0, 1.0) } else { 1.0 - v.clamp(0.0, 1.0) };
            mods.amp *= lerp(1.0, v01, l.amp.abs());
        }
        if self.fresh {
            self.prev = mods;
            self.fresh = false;
        }

        // --- Pitch --------------------------------------------------------------------------
        let tau = (snap.glide_ms * 0.001 / 4.6).max(0.0);
        let glide_k = if tau < 1e-4 { 1.0 } else { 1.0 - (-block_secs / tau).exp() };
        self.pitch += (self.target - self.pitch) * glide_k;
        let off_target = step.map(|s| s.semi).unwrap_or(0.0);
        let off_k = 1.0 - (-block_secs / tau.max(0.004)).exp();
        self.pitch_off += (off_target - self.pitch_off) * off_k;
        let midi = self.pitch + self.pitch_off + self.bend * snap.bend_range + mods.pitch;
        let hz1 = midi_to_hz(midi);
        let hz2 = midi_to_hz(midi + snap.semi2 + snap.fine2 / 100.0);
        let hz_sub = hz1 * (snap.sub_oct as f32).exp2();
        self.uni.configure(hz1, isr, snap.voices, snap.detune, snap.spread);

        // --- Per-block constants ------------------------------------------------------------
        let a = &snap.amp_env;
        let f = &snap.filt_env;
        let vel_gain = (1.0 - snap.vel_sens) + snap.vel_sens * self.vel;
        let ctx = Ctx {
            snap,
            prev: self.prev,
            now: mods,
            amp_c: AdsrCoefs::new(a.a, a.d, a.s, a.r, isr),
            fil_c: AdsrCoefs::new(f.a, f.d, f.s, f.r, isr),
            inc2: (hz2 / isr).clamp(0.0, 0.45),
            inc_sub: (hz_sub / isr).clamp(0.0, 0.45),
            base_cut: snap.cutoff * (snap.keytrack * (self.pitch - 60.0) / 12.0).exp2(),
            tone: ToneCoefs::new(snap.low_cut, snap.high_cut, isr),
            out_gain: OUT_SCALE * vel_gain,
            gate_target: if step.map(|s| s.on).unwrap_or(true) { 1.0 } else { 0.0 },
            gate_k: 1.0 - (-1.0 / (0.002 * isr)).exp(),
            n_i: (OS * n) as f32,
            isr,
            pre_gain: 1.0 + snap.filt_drive * 5.0,
            pre_norm: 1.0 / (0.5 + 0.5 * (1.0 + snap.filt_drive * 5.0)).sqrt(),
            side_k: 1.0 - (-std::f32::consts::TAU * 150.0 / isr).exp(),
        };

        for s in 0..n {
            let (l0, r0) = self.tick(&ctx, OS * s);
            let (l1, r1) = self.tick(&ctx, OS * s + 1);
            let (l2, r2) = self.tick(&ctx, OS * s + 2);
            let (l3, r3) = self.tick(&ctx, OS * s + 3);
            let (la, ra) = (self.dec_a[0].process(l0, l1), self.dec_a[1].process(r0, r1));
            let (lb, rb) = (self.dec_a[0].process(l2, l3), self.dec_a[1].process(r2, r3));
            out_l[s] = self.dec_b[0].process(la, lb);
            out_r[s] = self.dec_b[1].process(ra, rb);
        }

        self.prev = mods;
        if self.gate || !self.amp_env.is_idle() {
            self.note_beats += dt_beats as f64;
        }
    }

    /// One oversampled stereo sample.
    #[inline]
    fn tick(&mut self, c: &Ctx, i: usize) -> (f32, f32) {
        let snap = c.snap;
        let t = (i + 1) as f32 / c.n_i;
        let m = Mods::lerp(&c.prev, &c.now, t);

        let env_a = self.amp_env.next(&c.amp_c);
        let env_f = self.filt_env.next(&c.fil_c);
        self.gate_gain += (c.gate_target - self.gate_gain) * c.gate_k;

        // Oscillators
        let m2 = self.osc2.tick(snap.wave2, c.inc2);
        let fm = (snap.fm + m.fm).clamp(0.0, 1.0);
        let (o1l, o1r) = self.uni.tick(snap.wave1, (snap.pwm + m.pwm).clamp(0.05, 0.95), fm * 0.8 * m2);
        let noise = self.rng.next_bipolar() * snap.noise * 0.5;
        let sub = self.sub.tick(snap.sub_wave, c.inc_sub) * snap.sub_level;

        let o2 = m2 * snap.level2;
        let mut xl = o1l * snap.level1 + o2 + noise;
        let mut xr = o1r * snap.level1 + o2 + noise;
        let mut clean_sub = 0.0;
        if snap.sub_to_filter {
            xl += sub;
            xr += sub;
        } else {
            clean_sub = sub;
        }

        // Pre-drive and filter (cutoff modulated at audio-rate resolution)
        xl = fast_tanh(xl * c.pre_gain) * c.pre_norm;
        xr = fast_tanh(xr * c.pre_gain) * c.pre_norm;
        let oct = m.cut_oct + env_f * snap.env_amt * 8.0;
        let coefs = Coefs::new(c.base_cut * oct.clamp(-12.0, 12.0).exp2(), snap.res + m.res, c.isr);
        let mut yl = self.flt[0].process(snap.filt_kind, xl, &coefs) + clean_sub;
        let mut yr = self.flt[1].process(snap.filt_kind, xr, &coefs) + clean_sub;

        // Distortion A
        let da = &snap.dist_a;
        if da.kind != DistKind::Off {
            let drive = da.drive + m.drive;
            yl = lerp(yl, shape(da.kind, yl, drive), da.mix);
            yr = lerp(yr, shape(da.kind, yr, drive), da.mix);
        }

        // Formant (vowel) filter
        if snap.fmt_mix > 0.001 {
            // The vowel moves slowly compared to the 4x rate: refresh its coefficients once per
            // host sample.
            if i % OS == 0 {
                self.fmt_coefs = FormantCoefs::new(snap.fmt_morph + m.formant, snap.fmt_q, c.isr);
            }
            let fc = self.fmt_coefs;
            let wl = self.fmt[0].process(yl, &fc);
            let wr = self.fmt[1].process(yr, &fc);
            yl = lerp(yl, wl, snap.fmt_mix);
            yr = lerp(yr, wr, snap.fmt_mix);
        }

        // Distortion B
        let db = &snap.dist_b;
        if db.kind != DistKind::Off {
            yl = lerp(yl, shape(db.kind, yl, db.drive), db.mix);
            yr = lerp(yr, shape(db.kind, yr, db.drive), db.mix);
        }

        // Tone, then amplitude
        yl = self.tone[0].process(yl, &c.tone);
        yr = self.tone[1].process(yr, &c.tone);

        // Multiband compression sits before the amplifier, so it shapes the tone without pumping
        // up release tails.
        if snap.ott > 0.001 {
            (yl, yr) = self.ott.process(yl, yr, snap.ott);
        }

        let amp = env_a * self.gate_gain * m.amp * c.out_gain;
        yl *= amp;
        yr *= amp;

        // Keep everything below ~150 Hz mono: wide unison must not smear the sub.
        let mid = 0.5 * (yl + yr);
        let side = 0.5 * (yl - yr);
        self.side_lp += (side - self.side_lp) * c.side_k;
        let side = side - self.side_lp;
        (mid + side, mid - side)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn render_secs(e: &mut Engine, snap: &Snap, tr: &Transport, secs: f32) -> Vec<f32> {
        let blocks = (secs * SR) as usize / MAXB;
        let mut out = Vec::with_capacity(blocks * MAXB);
        for _ in 0..blocks {
            let mut l = [0.0f32; MAXB];
            let mut r = [0.0f32; MAXB];
            e.render(snap, tr, &mut l, &mut r);
            out.extend_from_slice(&l);
        }
        out
    }

    fn rms_series(x: &[f32], window: usize) -> Vec<f32> {
        x.chunks(window).map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt()).collect()
    }

    fn default_snap() -> Snap {
        Snap::read(&WubParams::default())
    }

    #[test]
    fn plays_and_stays_sane() {
        let snap = default_snap();
        let mut e = Engine::new(SR);
        e.note_on(36, 0.9, &snap);
        let x = render_secs(&mut e, &snap, &Transport::default(), 1.0);
        let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(x.iter().all(|v| v.is_finite()));
        assert!(peak > 0.05, "silent: {peak}");
        assert!(peak < 1.5, "too loud: {peak}");
        e.note_off(36, );
        render_secs(&mut e, &snap, &Transport::default(), 1.0);
        assert!(!e.is_sounding());
    }

    #[test]
    fn wobble_period_follows_the_tempo() {
        // 1/8 notes at 120 BPM = 4 wubs per second = 250 ms per cycle.
        let mut snap = default_snap();
        snap.lfo[0].cut = 1.0;
        snap.cutoff = 120.0;
        snap.res = 0.3;
        snap.sub_level = 0.0;
        snap.level2 = 0.0;
        let tr = Transport { tempo: 120.0, playing: false, beat: None };
        let mut e = Engine::new(SR);
        e.note_on(36, 1.0, &snap);
        let x = render_secs(&mut e, &snap, &tr, 3.0);
        let win = 480; // 10 ms
        let env = rms_series(&x[(SR as usize / 2)..], win);
        let mean = env.iter().sum::<f32>() / env.len() as f32;
        let dev: Vec<f32> = env.iter().map(|v| v - mean).collect();
        let mut best = (0usize, f32::MIN);
        for lag in 10..40 {
            let c: f32 = (0..dev.len() - lag).map(|i| dev[i] * dev[i + lag]).sum();
            if c > best.1 {
                best = (lag, c);
            }
        }
        assert!((23..=27).contains(&best.0), "autocorrelation peak at {} windows (expect ~25)", best.0);
    }

    #[test]
    fn pattern_rests_are_silent_and_steps_play() {
        let mut snap = default_snap();
        snap.pat_on = true;
        snap.pat_len = 4;
        snap.step_beats = 1.0; // 1/4 note steps -> 0.5 s each at 120 BPM
        for (i, on) in [true, false, true, false].into_iter().enumerate() {
            snap.steps[i].on = on;
        }
        snap.lfo[0].cut = 0.0; // keep the level steady
        let tr = Transport { tempo: 120.0, playing: false, beat: None };
        let mut e = Engine::new(SR);
        e.note_on(36, 1.0, &snap);
        let x = render_secs(&mut e, &snap, &tr, 2.0);
        let seg = |a: f32, b: f32| {
            let s = &x[(a * SR) as usize..(b * SR) as usize];
            (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()
        };
        let on1 = seg(0.1, 0.45);
        let rest1 = seg(0.6, 0.95);
        let on2 = seg(1.1, 1.45);
        assert!(on1 > 0.02 && on2 > 0.02, "steps should sound: {on1} {on2}");
        assert!(rest1 < on1 * 0.05, "rest not silent: {rest1} vs {on1}");
    }

    #[test]
    fn every_setting_combination_stays_finite() {
        use nih_plug::prelude::Enum;
        let base = default_snap();
        for fk in 0..3 {
            for dk in 0..7 {
                let mut snap = base;
                snap.filt_kind = FilterKind::from_index(fk);
                snap.dist_a.kind = DistKind::from_index(dk);
                snap.dist_a.drive = 1.0;
                snap.dist_b.kind = DistKind::from_index((dk + 3) % 7);
                snap.dist_b.drive = 1.0;
                snap.res = 1.0;
                snap.filt_drive = 1.0;
                snap.fmt_mix = 1.0;
                snap.fmt_q = 1.0;
                snap.fm = 1.0;
                snap.level2 = 1.0;
                snap.voices = 7;
                snap.lfo[0].cut = 1.0;
                snap.lfo[1].formant = 1.0;
                snap.lfo[1].fm = 1.0;
                snap.lfo[1].pitch = 1.0;
                let mut e = Engine::new(SR);
                e.note_on(24, 1.0, &snap);
                let x = render_secs(&mut e, &snap, &Transport::default(), 0.3);
                let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                assert!(x.iter().all(|v| v.is_finite()), "filter {fk} dist {dk} non-finite");
                assert!(peak < 40.0, "filter {fk} dist {dk} peak {peak}");
            }
        }
    }

    #[test]
    fn legato_does_not_retrigger_and_mono_returns_to_held_note() {
        let mut snap = default_snap();
        snap.legato = true;
        let mut e = Engine::new(SR);
        e.note_on(36, 1.0, &snap);
        render_secs(&mut e, &snap, &Transport::default(), 0.2);
        e.note_on(43, 1.0, &snap);
        assert_eq!(e.target, 43.0);
        e.note_off(43);
        assert_eq!(e.target, 36.0, "should fall back to the still-held note");
        e.note_off(36);
        render_secs(&mut e, &snap, &Transport::default(), 1.0);
        assert!(!e.is_sounding());
    }
}
