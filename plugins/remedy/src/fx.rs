//! Master effects rack: distortion (runs oversampled, inside the engine) -> chorus -> delay -> reverb.

use crate::params::{DistKind, SzParams};
use crate::util::fast_tanh;
use std::f32::consts::TAU;

// ---------------------------------------------------------------------------------------------
// Distortion
// ---------------------------------------------------------------------------------------------

/// Waveshaper used by the master distortion. Called at the oversampled rate by the engine.
#[inline]
pub fn distort(kind: DistKind, x: f32, drive: f32) -> f32 {
    let g = 1.0 + drive * drive * 30.0;
    match kind {
        DistKind::Soft => fast_tanh(x * g) / fast_tanh(g.min(3.0)).max(0.3) * 0.8,
        DistKind::Hard => (x * g).clamp(-1.0, 1.0) * 0.8,
        DistKind::Fold => {
            let u = (x * (1.0 + drive * 8.0) + 1.0).rem_euclid(4.0);
            (1.0 - (u - 2.0).abs()) * 0.8
        }
        DistKind::Crush => {
            let levels = (2.0f32).powf(2.0 + (1.0 - drive) * 10.0);
            (x * levels * 0.5).round() / (levels * 0.5)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Delay line helper
// ---------------------------------------------------------------------------------------------

struct DelayLine {
    buf: Vec<f32>,
    pos: usize,
    mask: usize,
}

impl DelayLine {
    fn new(min_len: usize) -> Self {
        let len = (min_len + 4).next_power_of_two().max(16);
        Self { buf: vec![0.0; len], pos: 0, mask: len - 1 }
    }

    #[inline]
    fn push(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) & self.mask;
    }

    /// The sample written `delay` pushes ago (1 = the most recent one).
    #[inline]
    fn tap(&self, delay: usize) -> f32 {
        self.buf[(self.pos + self.mask + 1 - delay.clamp(1, self.mask)) & self.mask]
    }

    /// Read `delay` samples behind the write head (linear interpolation).
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let d = delay.clamp(1.0, (self.mask - 2) as f32);
        let rp = self.pos as f32 - d + (self.mask + 1) as f32;
        let i = rp as usize;
        let f = rp - i as f32;
        let a = self.buf[i & self.mask];
        let b = self.buf[(i + 1) & self.mask];
        a + (b - a) * f
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Plate reverb (after Dattorro, "Effect Design Part 1", 1997)
// ---------------------------------------------------------------------------------------------
//
// Four input diffusers feed a figure-of-eight "tank" of two modulated all-passes, four delays and
// two more all-passes. Unlike a bank of parallel combs it has no fixed metallic pitch: the tail is
// dense and smooth, and the slow modulation keeps it from ringing.

struct Plate {
    k: f32,
    pre: DelayLine,
    in_ap: [DelayLine; 4],
    mod_ap: [DelayLine; 2],
    del_a: [DelayLine; 2],
    tank_ap: [DelayLine; 2],
    del_b: [DelayLine; 2],
    damp: [f32; 2],
    hp: f32,
    lfo: f32,
    tank_out: [f32; 2],
}

const IN_AP: [usize; 4] = [142, 107, 379, 277];
const MOD_AP: [usize; 2] = [672, 908];
const DEL_A: [usize; 2] = [4453, 4217];
const TANK_AP: [usize; 2] = [1800, 2656];
const DEL_B: [usize; 2] = [3720, 3163];

impl Plate {
    fn new(sr: f32) -> Self {
        let k = sr / 29_761.0;
        let dl = |n: usize| DelayLine::new((n as f32 * k) as usize + 64);
        Self {
            k,
            pre: DelayLine::new((0.05 * sr) as usize),
            in_ap: [dl(IN_AP[0]), dl(IN_AP[1]), dl(IN_AP[2]), dl(IN_AP[3])],
            mod_ap: [dl(MOD_AP[0]), dl(MOD_AP[1])],
            del_a: [dl(DEL_A[0]), dl(DEL_A[1])],
            tank_ap: [dl(TANK_AP[0]), dl(TANK_AP[1])],
            del_b: [dl(DEL_B[0]), dl(DEL_B[1])],
            damp: [0.0; 2],
            hp: 0.0,
            lfo: 0.0,
            tank_out: [0.0; 2],
        }
    }

    #[inline]
    fn len(&self, n: usize) -> usize {
        ((n as f32 * self.k) as usize).max(1)
    }

    /// Mono in, stereo out. `decay` 0..1 (feedback), `damping` 0..1 (high-frequency loss).
    #[inline]
    fn process(&mut self, x: f32, decay: f32, damping: f32, sr: f32) -> (f32, f32) {
        // Pre-delay and a gentle low cut so the tail does not turn to mud.
        self.pre.push(x);
        let mut v = self.pre.tap((0.012 * sr) as usize);
        self.hp += (v - self.hp) * (TAU * 140.0 / sr).min(0.5);
        v -= self.hp;

        // Input diffusion
        for (i, ap) in self.in_ap.iter_mut().enumerate() {
            let g = if i < 2 { 0.75 } else { 0.625 };
            let d = ap.tap(((IN_AP[i] as f32 * self.k) as usize).max(1));
            let w = v - g * d;
            ap.push(w);
            v = d + g * w;
        }

        self.lfo += 0.7 / sr;
        if self.lfo >= 1.0 {
            self.lfo -= 1.0;
        }
        let excursion = 12.0 * self.k;
        let mut out = [0.0f32; 2];
        let feed = [self.tank_out[1], self.tank_out[0]];
        for h in 0..2 {
            let mut a = v + decay * feed[h];
            // Modulated all-pass (decay diffusion 1, inverted)
            let phase = self.lfo + if h == 0 { 0.0 } else { 0.25 };
            let md = (MOD_AP[h] as f32 * self.k) + excursion * (phase * TAU).sin();
            let d = self.mod_ap[h].read(md.max(2.0));
            let w = a + 0.7 * d;
            self.mod_ap[h].push(w);
            a = d - 0.7 * w;
            // Long delay, damping, decay
            self.del_a[h].push(a);
            a = self.del_a[h].tap(self.len(DEL_A[h]));
            self.damp[h] += (a - self.damp[h]) * (1.0 - damping);
            a = self.damp[h] * decay;
            // All-pass (decay diffusion 2)
            let d = self.tank_ap[h].tap(self.len(TANK_AP[h]));
            let w = a - 0.5 * d;
            self.tank_ap[h].push(w);
            a = d + 0.5 * w;
            self.del_b[h].push(a);
            out[h] = self.del_b[h].tap(self.len(DEL_B[h]));
        }
        self.tank_out = out;

        let t = |line: &DelayLine, n: usize, k: f32| line.tap(((n as f32 * k) as usize).max(1));
        let k = self.k;
        let l = t(&self.del_a[1], 266, k) + t(&self.del_a[1], 2974, k) - t(&self.tank_ap[1], 1913, k)
            + t(&self.del_b[1], 1996, k)
            - t(&self.del_a[0], 1990, k)
            - t(&self.tank_ap[0], 187, k)
            - t(&self.del_b[0], 1066, k);
        let r = t(&self.del_a[0], 353, k) + t(&self.del_a[0], 3627, k) - t(&self.tank_ap[0], 1228, k)
            + t(&self.del_b[0], 2673, k)
            - t(&self.del_a[1], 2111, k)
            - t(&self.tank_ap[1], 335, k)
            - t(&self.del_b[1], 121, k);
        (l * 0.6, r * 0.6)
    }
}

// ---------------------------------------------------------------------------------------------
// The rack
// ---------------------------------------------------------------------------------------------

pub struct FxChain {
    sr: f32,
    chorus_l: DelayLine,
    chorus_r: DelayLine,
    chorus_phase: f32,
    delay_l: DelayLine,
    delay_r: DelayLine,
    delay_time: f32,
    delay_lp: [f32; 2],
    delay_hp: [f32; 2],
    plate: Plate,
}

impl FxChain {
    pub fn new(sr: f32) -> Self {
        let chorus_len = (0.06 * sr) as usize;
        let delay_len = (2.1 * sr) as usize;
        Self {
            sr,
            chorus_l: DelayLine::new(chorus_len),
            chorus_r: DelayLine::new(chorus_len),
            chorus_phase: 0.0,
            delay_l: DelayLine::new(delay_len),
            delay_r: DelayLine::new(delay_len),
            delay_time: 0.35 * sr,
            delay_lp: [0.0; 2],
            delay_hp: [0.0; 2],
            plate: Plate::new(sr),
        }
    }

    pub fn reset(&mut self) {
        self.chorus_l.clear();
        self.chorus_r.clear();
        self.delay_l.clear();
        self.delay_r.clear();
        self.delay_lp = [0.0; 2];
        self.delay_hp = [0.0; 2];
        self.plate = Plate::new(self.sr);
    }

    pub fn process(&mut self, p: &SzParams, l: &mut [f32], r: &mut [f32]) {
        let n = l.len().min(r.len());
        let sr = self.sr;

        // Chorus: three taps per side on spread LFO phases, added to the dry signal so it widens
        // and thickens without hollowing out the centre.
        if p.chorus.on.value() {
            let inc = p.chorus.rate.value() / sr;
            let depth = p.chorus.depth.value();
            let mix = p.chorus.mix.value();
            let swing = 0.004 * sr * depth;
            const BASE: [f32; 3] = [0.007, 0.0115, 0.016];
            const PH_L: [f32; 3] = [0.0, 0.33, 0.67];
            const PH_R: [f32; 3] = [0.5, 0.83, 0.17];
            for i in 0..n {
                let (xl, xr) = (l[i], r[i]);
                self.chorus_l.push(xl);
                self.chorus_r.push(xr);
                let (mut wl, mut wr) = (0.0, 0.0);
                for t in 0..3 {
                    let ml = ((self.chorus_phase + PH_L[t]) * TAU).sin();
                    let mr = ((self.chorus_phase + PH_R[t]) * TAU).sin();
                    wl += self.chorus_l.read(BASE[t] * sr + swing * ml);
                    wr += self.chorus_r.read(BASE[t] * sr + swing * mr);
                }
                let dry = 1.0 - 0.35 * mix;
                l[i] = xl * dry + wl * (0.55 * mix);
                r[i] = xr * dry + wr * (0.55 * mix);
                self.chorus_phase += inc;
                if self.chorus_phase >= 1.0 {
                    self.chorus_phase -= 1.0;
                }
            }
        }

        // Delay, with the repeats band-limited so they sit behind the dry sound.
        if p.delay.on.value() {
            let target = (p.delay.time.value() * 0.001 * sr).max(1.0);
            let fb = p.delay.feedback.value().min(0.95);
            let pp = p.delay.pingpong.value();
            let mix = p.delay.mix.value();
            let slew = 1.0 - (-1.0 / (0.05 * sr)).exp(); // ~50 ms smoothing of the time knob
            let damp = 1.0 - (-TAU * 5000.0 / sr).exp();
            let lowcut = 1.0 - (-TAU * 180.0 / sr).exp();
            for i in 0..n {
                self.delay_time += (target - self.delay_time) * slew;
                let dl = self.delay_l.read(self.delay_time);
                let dr = self.delay_r.read(self.delay_time);
                self.delay_lp[0] += (dl - self.delay_lp[0]) * damp;
                self.delay_lp[1] += (dr - self.delay_lp[1]) * damp;
                self.delay_hp[0] += (self.delay_lp[0] - self.delay_hp[0]) * lowcut;
                self.delay_hp[1] += (self.delay_lp[1] - self.delay_hp[1]) * lowcut;
                let (fl, fr) = (self.delay_lp[0] - self.delay_hp[0], self.delay_lp[1] - self.delay_hp[1]);
                let (xl, xr) = (l[i], r[i]);
                if pp {
                    let mono = 0.5 * (xl + xr);
                    self.delay_l.push(mono + fr * fb);
                    self.delay_r.push(fl * fb);
                } else {
                    self.delay_l.push(xl + fl * fb);
                    self.delay_r.push(xr + fr * fb);
                }
                l[i] = xl + dl * mix;
                r[i] = xr + dr * mix;
            }
        }

        // Reverb
        if p.reverb.on.value() {
            let size = p.reverb.size.value().clamp(0.0, 1.0);
            let decay = 0.3 + 0.62 * size.powf(0.7);
            let damping = 0.05 + 0.75 * p.reverb.damp.value().clamp(0.0, 1.0);
            let width = p.reverb.width.value();
            let mix = p.reverb.mix.value();
            let (w1, w2) = (0.5 * (1.0 + width), 0.5 * (1.0 - width));
            for i in 0..n {
                let (wl, wr) = self.plate.process(0.5 * (l[i] + r[i]), decay, damping, sr);
                let dry = 1.0 - 0.4 * mix;
                l[i] = l[i] * dry + (wl * w1 + wr * w2) * mix * 1.6;
                r[i] = r[i] * dry + (wr * w1 + wl * w2) * mix * 1.6;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plate_rings_out_and_stays_bounded() {
        let sr = 48_000.0;
        let mut p = Plate::new(sr);
        let mut peak = 0.0f32;
        let mut tail = 0.0f32;
        for i in 0..(sr as usize * 6) {
            let x = if i < 2400 { ((i * 7919) % 200) as f32 / 100.0 - 1.0 } else { 0.0 };
            let (l, r) = p.process(x, 0.92, 0.3, sr);
            assert!(l.is_finite() && r.is_finite());
            peak = peak.max(l.abs()).max(r.abs());
            if i > sr as usize * 5 {
                tail = tail.max(l.abs());
            }
        }
        assert!(peak > 0.05 && peak < 4.0, "plate peak {peak}");
        assert!(tail < peak * 0.3, "plate does not decay: {tail} vs {peak}");
    }
}
