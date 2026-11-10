//! Band-limited (PolyBLEP) oscillators: a unison main oscillator, a second oscillator that doubles
//! as FM modulator, and a sub oscillator.

use crate::params::{SubWave, Wave1, Wave2};
use plugin_common::dsp::Rng;
use std::f32::consts::{FRAC_PI_4, TAU};

pub const MAX_UNISON: usize = 7;

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

#[inline]
fn saw(p: f32, dt: f32) -> f32 {
    2.0 * p - 1.0 - blep(p, dt)
}

#[inline]
fn pulse(p: f32, dt: f32, width: f32) -> f32 {
    let naive = if p < width { 1.0 } else { -1.0 };
    let mut q = p - width;
    if q < 0.0 {
        q += 1.0;
    }
    naive + blep(p, dt) - blep(q, dt)
}

#[inline]
fn triangle(p: f32) -> f32 {
    4.0 * (p - 0.5).abs() - 1.0
}

#[inline]
fn wrap(p: f32) -> f32 {
    p - p.floor()
}

// ---------------------------------------------------------------------------------------------
// Main oscillator with unison
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct Unison {
    phase: [f32; MAX_UNISON],
    inc: [f32; MAX_UNISON],
    gl: [f32; MAX_UNISON],
    gr: [f32; MAX_UNISON],
    n: usize,
}

impl Default for Unison {
    fn default() -> Self {
        Self { phase: [0.0; MAX_UNISON], inc: [0.0; MAX_UNISON], gl: [0.0; MAX_UNISON], gr: [0.0; MAX_UNISON], n: 1 }
    }
}

impl Unison {
    /// Restart the unison stack. `rand` = 0 gives identical, tight starts every note.
    pub fn reset(&mut self, rand: f32, rng: &mut Rng) {
        for p in self.phase.iter_mut() {
            *p = rand * rng.next_f32();
        }
    }

    /// Per-block setup: pitch, detune and stereo layout.
    pub fn configure(&mut self, freq: f32, sample_rate: f32, voices: usize, detune: f32, spread: f32) {
        let n = voices.clamp(1, MAX_UNISON);
        self.n = n;
        let cents_max = detune.clamp(0.0, 1.0).powf(1.4) * 50.0;
        let norm = std::f32::consts::SQRT_2 / (n as f32).sqrt();
        for i in 0..n {
            let off = if n == 1 { 0.0 } else { i as f32 / (n - 1) as f32 * 2.0 - 1.0 };
            let ratio = (off * cents_max * (std::f32::consts::LN_2 / 1200.0)).exp();
            self.inc[i] = (freq * ratio / sample_rate).clamp(0.0, 0.45);
            let pan = (off * spread.clamp(0.0, 1.0)).clamp(-1.0, 1.0);
            let angle = (pan + 1.0) * FRAC_PI_4;
            self.gl[i] = angle.cos() * norm;
            self.gr[i] = angle.sin() * norm;
        }
    }

    /// One stereo sample. `pm` is a phase modulation offset in cycles (used for FM).
    #[inline]
    pub fn tick(&mut self, wave: Wave1, pulse_width: f32, pm: f32) -> (f32, f32) {
        let (mut l, mut r) = (0.0, 0.0);
        for i in 0..self.n {
            let p = wrap(self.phase[i] + pm);
            let dt = self.inc[i];
            let y = match wave {
                Wave1::Saw => saw(p, dt),
                Wave1::Square => pulse(p, dt, 0.5),
                Wave1::Pulse => pulse(p, dt, pulse_width.clamp(0.05, 0.95)),
                Wave1::Triangle => triangle(p),
            };
            l += y * self.gl[i];
            r += y * self.gr[i];
            let np = self.phase[i] + dt;
            self.phase[i] = if np >= 1.0 { np - 1.0 } else { np };
        }
        (l, r)
    }
}

// ---------------------------------------------------------------------------------------------
// Second oscillator (also the FM modulator)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct Osc2 {
    phase: f32,
}

impl Osc2 {
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    #[inline]
    pub fn tick(&mut self, wave: Wave2, inc: f32) -> f32 {
        let p = self.phase;
        let y = match wave {
            Wave2::Saw => saw(p, inc),
            Wave2::Square => pulse(p, inc, 0.5),
            Wave2::Sine => (p * TAU).sin(),
            Wave2::Triangle => triangle(p),
        };
        let np = p + inc;
        self.phase = if np >= 1.0 { np - 1.0 } else { np };
        y
    }
}

// ---------------------------------------------------------------------------------------------
// Sub oscillator
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct Sub {
    phase: f32,
}

impl Sub {
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    #[inline]
    pub fn tick(&mut self, wave: SubWave, inc: f32) -> f32 {
        let p = self.phase;
        let y = match wave {
            SubWave::Sine => (p * TAU).sin(),
            SubWave::Triangle => triangle(p),
            SubWave::Square => pulse(p, inc, 0.5) * 0.8,
        };
        let np = p + inc;
        self.phase = if np >= 1.0 { np - 1.0 } else { np };
        y
    }
}
