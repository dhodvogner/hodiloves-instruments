//! "OTT"-style three-band upward + downward compressor.
//!
//! Each band (low / mid / high) is squeezed towards a target level: loud parts are turned down and
//! quiet parts are turned up. On a bass this pulls the fizz and growl detail forward and evens out
//! the body, which is a large part of the modern dubstep sound. The bands are split by subtraction,
//! so with the amount at zero the signal passes through bit-exact.

use std::f32::consts::PI;

const SPLIT_LOW: f32 = 130.0;
const SPLIT_HIGH: f32 = 2500.0;
/// Target level of each band in dB.
const TARGET: [f32; 3] = [-12.0, -16.0, -22.0];
const MAKEUP: [f32; 3] = [3.0, 4.0, 5.0];
const MAX_BOOST: f32 = 14.0;

#[derive(Clone, Copy, Default)]
struct Svf {
    s1: f32,
    s2: f32,
}

impl Svf {
    /// Returns (low-pass, high-pass) of a 12 dB Butterworth pair.
    #[inline]
    fn split(&mut self, x: f32, g: f32) -> (f32, f32) {
        let r2 = std::f32::consts::SQRT_2;
        let a1 = 1.0 / (1.0 + r2 * g + g * g);
        let hp = (x - (r2 + g) * self.s1 - self.s2) * a1;
        let bp = g * hp + self.s1;
        self.s1 = g * hp + bp;
        let lp = g * bp + self.s2;
        self.s2 = g * bp + lp;
        (lp, hp)
    }
}

#[derive(Clone)]
pub struct Ott {
    g_low: f32,
    g_high: f32,
    atk: f32,
    rel: f32,
    smooth: f32,
    low: [Svf; 2],
    high: [Svf; 2],
    env: [f32; 3],
    gain: [f32; 3],
    target: [f32; 3],
    count: u32,
}

impl Ott {
    pub fn new(sample_rate: f32) -> Self {
        let k = |secs: f32| 1.0 - (-1.0 / (secs * sample_rate)).exp();
        Self {
            g_low: (PI * SPLIT_LOW / sample_rate).tan(),
            g_high: (PI * SPLIT_HIGH / sample_rate).tan(),
            atk: k(0.004),
            rel: k(0.09),
            smooth: k(0.006),
            low: [Svf::default(); 2],
            high: [Svf::default(); 2],
            env: [0.0; 3],
            gain: [1.0; 3],
            target: [1.0; 3],
            count: 0,
        }
    }

    pub fn reset(&mut self) {
        self.low = [Svf::default(); 2];
        self.high = [Svf::default(); 2];
        self.env = [0.0; 3];
        self.gain = [1.0; 3];
        self.target = [1.0; 3];
    }

    fn gain_for(level: f32, band: usize, depth: f32) -> f32 {
        let db = 20.0 * level.max(1e-5).log10();
        let over = TARGET[band] - db;
        let mut g = if over < 0.0 {
            over * 0.6 // downward: about 2.5:1
        } else {
            // upward, fading out towards silence so noise and release tails are not pumped up
            (over * 0.55).min(MAX_BOOST) * ((db + 70.0) / 15.0).clamp(0.0, 1.0)
        };
        g += MAKEUP[band];
        10.0f32.powf(g * depth / 20.0)
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32, depth: f32) -> (f32, f32) {
        let (ll, _) = self.low[0].split(l, self.g_low);
        let (lr, _) = self.low[1].split(r, self.g_low);
        let (rest_l, rest_r) = (l - ll, r - lr);
        let (_, hl) = self.high[0].split(rest_l, self.g_high);
        let (_, hr) = self.high[1].split(rest_r, self.g_high);
        let (ml, mr) = (rest_l - hl, rest_r - hr);
        let bands = [(ll, lr), (ml, mr), (hl, hr)];

        for b in 0..3 {
            let level = bands[b].0.abs().max(bands[b].1.abs());
            let k = if level > self.env[b] { self.atk } else { self.rel };
            self.env[b] += (level - self.env[b]) * k;
        }
        self.count += 1;
        if self.count >= 16 {
            self.count = 0;
            for b in 0..3 {
                self.target[b] = Self::gain_for(self.env[b], b, depth);
            }
        }
        let (mut ol, mut or) = (0.0, 0.0);
        for b in 0..3 {
            self.gain[b] += (self.target[b] - self.gain[b]) * self.smooth;
            ol += bands[b].0 * self.gain[b];
            or += bands[b].1 * self.gain[b];
        }
        (ol, or)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_depth_is_transparent_and_full_depth_lifts_quiet_highs() {
        let sr = 96_000.0;
        let mut o = Ott::new(sr);
        for i in 0..20_000 {
            let x = (i as f32 * 0.01).sin() * 0.5;
            let (l, r) = o.process(x, x, 0.0);
            assert!((l - x).abs() < 1e-4 && (r - x).abs() < 1e-4);
        }
        // A quiet 6 kHz tone should come out louder with the effect fully on.
        let mut o = Ott::new(sr);
        let mut peak = 0.0f32;
        for i in 0..96_000 {
            let x = (std::f32::consts::TAU * 6000.0 * i as f32 / sr).sin() * 0.02;
            let (l, _) = o.process(x, x, 1.0);
            assert!(l.is_finite());
            if i > 48_000 {
                peak = peak.max(l.abs());
            }
        }
        assert!(peak > 0.04, "quiet highs were not lifted: {peak}");
        assert!(peak < 0.5, "quiet highs were lifted far too much: {peak}");
    }
}
