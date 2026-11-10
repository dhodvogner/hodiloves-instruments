//! Wavetable oscillator with up to 16 unison voices and two chained warp slots.

use crate::params::WarpMode;
use crate::util::{fast_tanh, Rng};
use crate::wavetable::Table;
use std::f32::consts::{FRAC_PI_4, TAU};

pub const MAX_UNISON: usize = 16;

/// One warp slot, with per-block precomputed helpers.
#[derive(Clone, Copy)]
pub struct Warp {
    pub mode: WarpMode,
    pub amt: f32,
    aux: f32,
    aux2: f32,
}

impl Warp {
    pub fn new(mode: WarpMode, amt: f32) -> Self {
        let amt = amt.clamp(0.0, 1.0);
        let (aux, aux2) = match mode {
            WarpMode::Fold => (1.0 + amt * 5.0, 0.0),
            WarpMode::Drive => {
                let g = 1.0 + amt * 8.0;
                (g, 1.0 / fast_tanh(g).max(0.1))
            }
            WarpMode::Crush => {
                let levels = (1.0 + (1.0 - amt) * 7.0).exp2();
                (levels, 1.0 / levels)
            }
            _ => (0.0, 0.0),
        };
        Self { mode, amt, aux, aux2 }
    }

    /// How much faster than the note this warp reads the table (used to pick a safe mip level).
    #[inline]
    fn speed(&self) -> f32 {
        match self.mode {
            WarpMode::Sync => 1.0 + self.amt * 7.0,
            WarpMode::BendPlus | WarpMode::BendMinus => 1.0 + self.amt * 3.0,
            WarpMode::Asym => 1.0 + self.amt * 2.0,
            WarpMode::Mirror => 1.0 + self.amt,
            WarpMode::FmSelf | WarpMode::Twist => 1.0 + self.amt * 1.5,
            WarpMode::Fold => 1.0 + self.amt * 3.0,
            WarpMode::Drive => 1.0 + self.amt,
            _ => 1.0,
        }
    }

    #[inline]
    fn is_phase(&self) -> bool {
        matches!(
            self.mode,
            WarpMode::Sync
                | WarpMode::BendPlus
                | WarpMode::BendMinus
                | WarpMode::Asym
                | WarpMode::Mirror
                | WarpMode::Twist
                | WarpMode::FmSelf
        )
    }

    #[inline]
    fn phase(&self, p: f32, prev: f32) -> f32 {
        let a = self.amt;
        let q = match self.mode {
            WarpMode::Sync => p * (1.0 + a * 7.0),
            WarpMode::BendPlus => p / (1.0 + a * 8.0 * (1.0 - p)),
            WarpMode::BendMinus => 1.0 - (1.0 - p) / (1.0 + a * 8.0 * p),
            WarpMode::Asym => {
                let s = 0.5 - 0.45 * a;
                if p < s {
                    0.5 * p / s
                } else {
                    0.5 + 0.5 * (p - s) / (1.0 - s)
                }
            }
            WarpMode::Mirror => {
                let m = if p < 0.5 { p * 2.0 } else { 2.0 - 2.0 * p };
                p + (m - p) * a
            }
            WarpMode::Twist => p + a * 0.25 * (p * TAU).sin(),
            WarpMode::FmSelf => p + a * 0.5 * prev,
            _ => p,
        };
        q - q.floor()
    }

    #[inline]
    fn output(&self, y: f32) -> f32 {
        match self.mode {
            WarpMode::Fold => {
                let x = y * self.aux;
                let u = (x + 1.0).rem_euclid(4.0);
                1.0 - (u - 2.0).abs()
            }
            WarpMode::Drive => fast_tanh(y * self.aux) * self.aux2,
            WarpMode::Crush => (y * 0.5 * self.aux).round() * self.aux2 * 2.0,
            _ => y,
        }
    }
}

/// Everything the oscillator needs for one control block.
pub struct OscBlock {
    pub freq: f32,
    /// Wavetable position at the start and end of the block (ramped per sample).
    pub pos0: f32,
    pub pos1: f32,
    pub unison: usize,
    pub detune: f32,
    pub blend: f32,
    pub spread: f32,
    pub warp: [Warp; 2],
    pub pan: f32,
    pub sample_rate: f32,
}

#[derive(Clone)]
pub struct Osc {
    phase: [f32; MAX_UNISON],
    prev: [f32; MAX_UNISON],
}

impl Default for Osc {
    fn default() -> Self {
        Self { phase: [0.0; MAX_UNISON], prev: [0.0; MAX_UNISON] }
    }
}

impl Osc {
    pub fn reset(&mut self, start_phase: f32, rand: f32, rng: &mut Rng) {
        for v in 0..MAX_UNISON {
            let p = start_phase + rand * rng.next_f32();
            self.phase[v] = p - p.floor();
            self.prev[v] = 0.0;
        }
    }

    /// Add this oscillator's output (at unit level) into `l` / `r`.
    pub fn render(&mut self, table: &Table, c: &OscBlock, l: &mut [f32], r: &mut [f32]) {
        let n = c.unison.clamp(1, MAX_UNISON);
        let len = l.len().min(r.len());
        let cents_max = c.detune.clamp(0.0, 1.0).powf(1.6) * 100.0;
        let dpos = if len > 0 { (c.pos1 - c.pos0) / len as f32 } else { 0.0 };
        let speed = c.warp[0].speed() * c.warp[1].speed();

        // Unison weights: centre-most voices are full level, outer voices follow "blend".
        let mut weights = [0.0f32; MAX_UNISON];
        let mut sumsq = 0.0;
        for v in 0..n {
            let dist = (v as f32 - (n as f32 - 1.0) * 0.5).abs();
            let w = if dist < 0.51 { 1.0 } else { c.blend.clamp(0.0, 1.0) };
            weights[v] = w;
            sumsq += w * w;
        }
        let norm = 1.0 / sumsq.sqrt().max(1e-6);
        let nyq = 0.5 * c.sample_rate;
        let [w1, w2] = c.warp;
        let phase_w1 = w1.is_phase();
        let phase_w2 = w2.is_phase();

        for v in 0..n {
            let off = if n == 1 { 0.0 } else { (v as f32 / (n - 1) as f32) * 2.0 - 1.0 };
            // Voices bunch up towards the centre pitch (like a hardware supersaw) instead of being
            // evenly spaced, which avoids the regular, "phasey" beating of a linear spread.
            let det = off.abs().powf(1.5).copysign(off);
            let ratio = (det * cents_max * (std::f32::consts::LN_2 / 1200.0)).exp();
            let freq = (c.freq * ratio).clamp(1.0, nyq * 0.45);
            let inc = freq / c.sample_rate;
            let mip = Table::mip_for(freq * speed, c.sample_rate);

            // Alternate sides so flat and sharp voices are spread evenly across the stereo field.
            let side = if v % 2 == 0 { -1.0 } else { 1.0 };
            let pan = (c.pan + c.spread.clamp(0.0, 1.0) * off.abs() * side).clamp(-1.0, 1.0);
            let angle = (pan + 1.0) * FRAC_PI_4;
            let g = weights[v] * norm;
            let (gl, gr) = (angle.cos() * g, angle.sin() * g);

            let mut ph = self.phase[v];
            let mut prev = self.prev[v];
            for s in 0..len {
                let mut p = ph;
                if phase_w1 {
                    p = w1.phase(p, prev);
                }
                if phase_w2 {
                    p = w2.phase(p, prev);
                }
                let mut y = table.sample(mip, c.pos0 + dpos * s as f32, p);
                if !phase_w1 {
                    y = w1.output(y);
                }
                if !phase_w2 {
                    y = w2.output(y);
                }
                prev = y;
                ph += inc;
                if ph >= 1.0 {
                    ph -= 1.0;
                }
                l[s] += y * gl;
                r[s] += y * gr;
            }
            self.phase[v] = ph;
            self.prev[v] = prev;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Sub oscillator (PolyBLEP saw / square)
// ---------------------------------------------------------------------------------------------

#[inline]
fn blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

#[derive(Clone, Default)]
pub struct SubOsc {
    pub phase: f32,
}

impl SubOsc {
    #[inline]
    pub fn next(&mut self, shape: crate::params::SubShape, inc: f32) -> f32 {
        use crate::params::SubShape::*;
        let t = self.phase;
        let y = match shape {
            Sine => (t * TAU).sin(),
            Triangle => 4.0 * (t - 0.5).abs() - 1.0,
            Saw => 2.0 * t - 1.0 - blep(t, inc),
            Square => {
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                naive + blep(t, inc) - blep((t + 0.5).fract(), inc)
            }
        };
        self.phase += inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        y
    }
}
