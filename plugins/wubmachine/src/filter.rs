//! Filters: resonant 24 dB ladder, 12 dB state-variable (low / band), a 3-band vowel formant bank
//! and the tone-shaping filters. Everything is stereo (two channel states) and runs at the
//! oversampled rate, with coefficients recomputed per sample so fast modulation stays smooth.

use crate::params::FilterKind;
use plugin_common::dsp::{lerp, Ladder};
use std::f32::consts::PI;

/// Filter coefficients shared by both channels for one sample.
#[derive(Clone, Copy)]
pub struct Coefs {
    g: f32,
    k: f32,
    r2: f32,
    a1: f32,
}

impl Coefs {
    pub fn new(cutoff: f32, res: f32, sample_rate: f32) -> Self {
        let fc = cutoff.clamp(15.0, (0.45 * sample_rate).min(20_000.0));
        let res = res.clamp(0.0, 1.0);
        let g = (PI * fc / sample_rate).tan();
        let q = 0.6 + 18.0 * res * res * res;
        let r2 = 1.0 / q;
        Self { g, k: 3.98 * res.powf(1.15), r2, a1: 1.0 / (1.0 + r2 * g + g * g) }
    }
}

#[derive(Clone, Default)]
pub struct FilterCh {
    s1: f32,
    s2: f32,
    ladder: Ladder,
}

impl FilterCh {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline]
    pub fn process(&mut self, kind: FilterKind, x: f32, c: &Coefs) -> f32 {
        let y = match kind {
            FilterKind::Low24 => self.ladder(x, c),
            FilterKind::Low12 => self.svf(x, c).2,
            FilterKind::Band12 => self.svf(x, c).1 * c.r2.min(1.5),
        };
        if y.is_finite() {
            y.clamp(-8.0, 8.0)
        } else {
            self.reset();
            0.0
        }
    }

    /// Returns (highpass, bandpass, lowpass).
    #[inline]
    fn svf(&mut self, x: f32, c: &Coefs) -> (f32, f32, f32) {
        let hp = (x - (c.r2 + c.g) * self.s1 - self.s2) * c.a1;
        let bp = c.g * hp + self.s1;
        self.s1 = c.g * hp + bp;
        let lp = c.g * bp + self.s2;
        self.s2 = c.g * bp + lp;
        (hp, bp, lp)
    }

    #[inline]
    fn ladder(&mut self, x: f32, c: &Coefs) -> f32 {
        // Resonance eats bass in a ladder; give most of it back so low wubs stay fat.
        self.ladder.process(x, c.g, c.k) * (1.0 + 0.5 * c.k)
    }
}

// ---------------------------------------------------------------------------------------------
// Formant (vowel) filter
// ---------------------------------------------------------------------------------------------

/// First three formants in Hz for A, E, I, O, U.
const VOWELS: [[f32; 3]; 5] = [
    [800.0, 1150.0, 2900.0],
    [400.0, 1600.0, 2700.0],
    [350.0, 1700.0, 2700.0],
    [450.0, 800.0, 2830.0],
    [325.0, 700.0, 2530.0],
];
const FORMANT_GAINS: [f32; 3] = [1.0, 0.7, 0.45];
const FORMANT_MAKEUP: f32 = 3.0;

#[derive(Clone, Copy)]
pub struct FormantCoefs {
    g: [f32; 3],
    a1: [f32; 3],
    r2: f32,
}

impl FormantCoefs {
    /// `morph` sweeps A -> E -> I -> O -> U (0..1); `q01` maps to Q 3..16.
    pub fn new(morph: f32, q01: f32, sample_rate: f32) -> Self {
        let pos = morph.clamp(0.0, 1.0) * 4.0;
        let i = (pos as usize).min(3);
        let f = pos - i as f32;
        let q = lerp(3.0, 16.0, q01.clamp(0.0, 1.0));
        let r2 = 1.0 / q;
        let mut g = [0.0; 3];
        let mut a1 = [0.0; 3];
        for k in 0..3 {
            let (a, b) = (VOWELS[i][k], VOWELS[i + 1][k]);
            let hz = (a.ln() * (1.0 - f) + b.ln() * f).exp().min(0.45 * sample_rate);
            g[k] = (PI * hz / sample_rate).tan();
            a1[k] = 1.0 / (1.0 + r2 * g[k] + g[k] * g[k]);
        }
        Self { g, a1, r2 }
    }
}

#[derive(Clone, Default)]
pub struct FormantCh {
    s1: [f32; 3],
    s2: [f32; 3],
}

impl FormantCh {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    #[inline]
    pub fn process(&mut self, x: f32, c: &FormantCoefs) -> f32 {
        let mut out = 0.0;
        for k in 0..3 {
            let hp = (x - (c.r2 + c.g[k]) * self.s1[k] - self.s2[k]) * c.a1[k];
            let bp = c.g[k] * hp + self.s1[k];
            self.s1[k] = c.g[k] * hp + bp;
            let lp = c.g[k] * bp + self.s2[k];
            self.s2[k] = c.g[k] * bp + lp;
            out += bp * c.r2 * FORMANT_GAINS[k];
        }
        out * FORMANT_MAKEUP
    }
}

// ---------------------------------------------------------------------------------------------
// Tone: 12 dB low cut (also removes DC) and a one-pole high cut
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub struct ToneCoefs {
    g: f32,
    r2: f32,
    a1: f32,
    lp: f32,
}

impl ToneCoefs {
    pub fn new(low_cut: f32, high_cut: f32, sample_rate: f32) -> Self {
        let g = (PI * low_cut.clamp(15.0, 1000.0) / sample_rate).tan();
        let r2 = std::f32::consts::SQRT_2;
        let lp = 1.0 - (-2.0 * PI * high_cut.clamp(500.0, 0.45 * sample_rate) / sample_rate).exp();
        Self { g, r2, a1: 1.0 / (1.0 + r2 * g + g * g), lp }
    }
}

#[derive(Clone, Default)]
pub struct ToneCh {
    s1: f32,
    s2: f32,
    lp: f32,
}

impl ToneCh {
    #[inline]
    pub fn process(&mut self, x: f32, c: &ToneCoefs) -> f32 {
        let hp = (x - (c.r2 + c.g) * self.s1 - self.s2) * c.a1;
        let bp = c.g * hp + self.s1;
        self.s1 = c.g * hp + bp;
        let lp = c.g * bp + self.s2;
        self.s2 = c.g * bp + lp;
        let _ = lp;
        self.lp += (hp - self.lp) * c.lp;
        self.lp
    }
}
