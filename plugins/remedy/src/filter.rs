//! Voice filter: TPT state-variable (12 dB), zero-delay ladder (24 dB) and feedback combs.
//!
//! The cutoff is ramped per sample across each control block, so envelope and LFO sweeps are
//! smooth instead of stepping at the control rate (which is heard as "zipper" buzz).

use crate::params::FilterKind;
use crate::util::{fast_tanh, lerp, Ladder};
use std::f32::consts::PI;

const COMB_LEN: usize = 8192;
const COMB_MASK: usize = COMB_LEN - 1;

/// Per-voice memory of where the last block's ramps ended.
#[derive(Clone, Copy)]
pub struct FilterRamp {
    pub g: f32,
    pub comb: f32,
}

impl Default for FilterRamp {
    fn default() -> Self {
        Self { g: f32::NAN, comb: f32::NAN }
    }
}

/// Coefficients shared by both channels of a voice for one control block.
#[derive(Clone, Copy)]
pub struct FilterBlock {
    pub kind: FilterKind,
    g0: f32,
    dg: f32,
    r2: f32,
    ladder_k: f32,
    ladder_gain: f32,
    comb0: f32,
    dcomb: f32,
    comb_fb: f32,
    drive_gain: f32,
    drive_norm: f32,
    pub mix: f32,
}

impl FilterBlock {
    /// `ramp` holds the previous block's end values and is updated to this block's targets.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: FilterKind,
        cutoff: f32,
        res: f32,
        drive: f32,
        mix: f32,
        sample_rate: f32,
        samples: usize,
        ramp: &mut FilterRamp,
    ) -> Self {
        let fc = cutoff.clamp(20.0, (0.45 * sample_rate).min(21_000.0));
        let res = res.clamp(0.0, 1.0);
        let g1 = (PI * fc / sample_rate).tan();
        let comb1 = (sample_rate / fc).clamp(2.0, (COMB_LEN - 4) as f32);
        let g0 = if ramp.g.is_finite() { ramp.g } else { g1 };
        let comb0 = if ramp.comb.is_finite() { ramp.comb } else { comb1 };
        ramp.g = g1;
        ramp.comb = comb1;
        let inv = 1.0 / samples.max(1) as f32;

        let q = 0.55 + 20.0 * res * res * res;
        let ladder_k = 3.98 * res.powf(1.15);
        let sign = if kind == FilterKind::CombMinus { -1.0 } else { 1.0 };
        let drive = drive.clamp(0.0, 1.0);
        let drive_gain = 1.0 + drive * drive * 15.0;
        Self {
            kind,
            g0,
            dg: (g1 - g0) * inv,
            r2: 1.0 / q,
            ladder_k,
            // A ladder loses passband level as resonance rises; give most of it back.
            ladder_gain: 1.0 + 0.55 * ladder_k,
            comb0,
            dcomb: (comb1 - comb0) * inv,
            comb_fb: sign * (0.1 + 0.87 * res),
            drive_gain,
            drive_norm: 1.0 / drive_gain.powf(0.6),
            mix: mix.clamp(0.0, 1.0),
        }
    }
}

#[derive(Clone)]
pub struct FilterChannel {
    s1: f32,
    s2: f32,
    ladder: Ladder,
    comb: Vec<f32>,
    comb_pos: usize,
}

impl Default for FilterChannel {
    fn default() -> Self {
        Self { s1: 0.0, s2: 0.0, ladder: Ladder::default(), comb: vec![0.0; COMB_LEN], comb_pos: 0 }
    }
}

impl FilterChannel {
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
        self.ladder.reset();
        self.comb.iter_mut().for_each(|v| *v = 0.0);
        self.comb_pos = 0;
    }

    /// Process sample number `i` of the block.
    #[inline]
    pub fn process(&mut self, x: f32, c: &FilterBlock, i: usize) -> f32 {
        let dry = x;
        let t = i as f32;
        let g = c.g0 + c.dg * t;
        let x = if c.drive_gain > 1.001 { fast_tanh(x * c.drive_gain) * c.drive_norm } else { x };
        let y = match c.kind {
            FilterKind::Lp24 => self.ladder.process(x, g, c.ladder_k) * c.ladder_gain,
            FilterKind::CombPlus | FilterKind::CombMinus => self.comb(x, c, c.comb0 + c.dcomb * t),
            _ => self.svf(x, c, g),
        };
        // Guard against blow-ups from extreme settings.
        let y = if y.is_finite() {
            y.clamp(-8.0, 8.0)
        } else {
            self.s1 = 0.0;
            self.s2 = 0.0;
            self.ladder.reset();
            0.0
        };
        lerp(dry, y, c.mix)
    }

    #[inline]
    fn svf(&mut self, x: f32, c: &FilterBlock, g: f32) -> f32 {
        let a1 = 1.0 / (1.0 + c.r2 * g + g * g);
        let hp = (x - (c.r2 + g) * self.s1 - self.s2) * a1;
        let bp = g * hp + self.s1;
        self.s1 = g * hp + bp;
        let lp = g * bp + self.s2;
        self.s2 = g * bp + lp;
        match c.kind {
            FilterKind::Lp12 => lp,
            FilterKind::Hp12 => hp,
            FilterKind::Bp12 => bp * c.r2.min(1.5),
            FilterKind::Notch => x - c.r2 * bp,
            FilterKind::Peak => lp - hp,
            _ => lp,
        }
    }

    #[inline]
    fn comb(&mut self, x: f32, c: &FilterBlock, delay: f32) -> f32 {
        let read = self.comb_pos as f32 - delay + COMB_LEN as f32;
        let i = read as usize;
        let frac = read - i as f32;
        let a = self.comb[i & COMB_MASK];
        let b = self.comb[(i + 1) & COMB_MASK];
        let delayed = a + (b - a) * frac;
        let y = x + c.comb_fb * delayed;
        self.comb[self.comb_pos] = y;
        self.comb_pos = (self.comb_pos + 1) & COMB_MASK;
        y * (1.0 - 0.5 * c.comb_fb.abs())
    }
}

/// Approximate magnitude response (linear gain) at `hz`, for the editor's filter display.
pub fn response(kind: FilterKind, cutoff: f32, res: f32, hz: f32) -> f32 {
    let res = res.clamp(0.0, 1.0);
    let w = (hz / cutoff.max(1.0)).max(1e-4);
    let w2 = w * w;
    let r = 1.0 / (0.55 + 20.0 * res * res * res);
    let den = ((1.0 - w2).powi(2) + (r * w).powi(2)).sqrt().max(1e-6);
    match kind {
        FilterKind::Lp12 => 1.0 / den,
        FilterKind::Hp12 => w2 / den,
        FilterKind::Bp12 => r.min(1.5) * w / den,
        FilterKind::Notch => (1.0 - w2).abs() / den,
        FilterKind::Peak => (1.0 + w2) / den,
        FilterKind::Lp24 => {
            // Four one-poles inside a feedback loop of gain k: H = G^4 / (1 + k G^4).
            let k = 3.98 * res.powf(1.15);
            let (re1, im1) = (1.0 / (1.0 + w2), -w / (1.0 + w2));
            let (re2, im2) = (re1 * re1 - im1 * im1, 2.0 * re1 * im1);
            let (re4, im4) = (re2 * re2 - im2 * im2, 2.0 * re2 * im2);
            let (dr, di) = (1.0 + k * re4, k * im4);
            let mag = (re4 * re4 + im4 * im4).sqrt() / (dr * dr + di * di).sqrt().max(1e-6);
            mag * (1.0 + 0.55 * k)
        }
        FilterKind::CombPlus | FilterKind::CombMinus => {
            let fb = (0.1 + 0.87 * res) * if kind == FilterKind::CombMinus { -1.0 } else { 1.0 };
            let ph = 2.0 * PI * w;
            let (dr, di) = (1.0 - fb * ph.cos(), fb * ph.sin());
            (1.0 - 0.5 * fb.abs()) / (dr * dr + di * di).sqrt().max(1e-6)
        }
    }
}
