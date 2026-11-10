//! Small DSP helpers shared by every module.

/// Tiny xorshift32 PRNG; allocation free and good enough for audio noise / random phases.
#[derive(Clone)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in [-1, 1).
    #[inline]
    pub fn next_bipolar(&mut self) -> f32 {
        self.next_f32() * 2.0 - 1.0
    }
}

/// Rational tanh approximation, exact at the clamp boundary (|x| = 3 -> 1).
#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    let x = x.clamp(-3.0, 3.0);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[inline]
pub fn midi_to_hz(note: f32) -> f32 {
    440.0 * ((note - 69.0) * (1.0 / 12.0)).exp2()
}

/// Smooth output limiter: linear below the knee, tanh-compressed above it.
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.7;
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let y = KNEE + (1.0 - KNEE) * fast_tanh((a - KNEE) / (1.0 - KNEE));
        y.copysign(x)
    }
}

#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ---------------------------------------------------------------------------------------------
// Zero-delay-feedback ladder filter (24 dB/oct low-pass)
// ---------------------------------------------------------------------------------------------

/// Four trapezoidal one-pole stages with the feedback loop solved analytically, so the resonance
/// stays in tune and stable right up to self-oscillation, even while the cutoff is moving fast.
/// The loop input is saturated, which rounds the resonance off the way an analog ladder does.
#[derive(Clone, Copy, Default)]
pub struct Ladder {
    s: [f32; 4],
}

impl Ladder {
    pub fn reset(&mut self) {
        self.s = [0.0; 4];
    }

    /// `g` = tan(pi * cutoff / sample_rate), `k` = feedback 0..4 (4 = self-oscillation).
    #[inline]
    pub fn process(&mut self, x: f32, g: f32, k: f32) -> f32 {
        let big = g / (1.0 + g);
        let b2 = big * big;
        let sigma = (1.0 - big) * (b2 * big * self.s[0] + b2 * self.s[1] + big * self.s[2] + self.s[3]);
        let u = (x - k * sigma) / (1.0 + k * b2 * b2);
        let mut v = fast_tanh(u);
        for s in self.s.iter_mut() {
            let d = (v - *s) * big;
            let y = d + *s;
            *s = y + d;
            v = y;
        }
        v
    }
}

/// One-pole DC blocker (about 5 Hz at 48 kHz).
#[derive(Clone, Copy, Default)]
pub struct DcBlock {
    x1: f32,
    y1: f32,
}

impl DcBlock {
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = x - self.x1 + 0.9993 * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
}

// ---------------------------------------------------------------------------------------------
// ADSR envelope
// ---------------------------------------------------------------------------------------------

/// ADSR timing converted to per-sample coefficients (computed once per control block).
#[derive(Clone, Copy, Default)]
pub struct AdsrCoefs {
    attack_inc: f32,
    decay_coef: f32,
    sustain: f32,
    release_coef: f32,
}

impl AdsrCoefs {
    pub fn new(attack_ms: f32, decay_ms: f32, sustain: f32, release_ms: f32, sample_rate: f32) -> Self {
        let samples = |ms: f32| (ms.max(0.1) * 0.001 * sample_rate).max(1.0);
        // Exponential segments reach ~1% of the target in the stated time (4.6 time constants).
        Self {
            attack_inc: 1.0 / samples(attack_ms),
            decay_coef: (-4.6 / samples(decay_ms)).exp(),
            sustain: sustain.clamp(0.0, 1.0),
            release_coef: (-4.6 / samples(release_ms)).exp(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone)]
pub struct Adsr {
    stage: Stage,
    value: f32,
}

impl Default for Adsr {
    fn default() -> Self {
        Self { stage: Stage::Idle, value: 0.0 }
    }
}

impl Adsr {
    pub fn is_idle(&self) -> bool {
        self.stage == Stage::Idle
    }

    /// Start (or restart) the attack from the current value, so retriggers do not click.
    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack;
    }

    pub fn gate_off(&mut self) {
        if self.stage != Stage::Idle {
            self.stage = Stage::Release;
        }
    }

    pub fn kill(&mut self) {
        self.stage = Stage::Idle;
        self.value = 0.0;
    }

    #[inline]
    pub fn next(&mut self, c: &AdsrCoefs) -> f32 {
        match self.stage {
            Stage::Idle => {}
            Stage::Attack => {
                self.value += c.attack_inc;
                if self.value >= 1.0 {
                    self.value = 1.0;
                    self.stage = Stage::Decay;
                }
            }
            Stage::Decay => {
                self.value = c.sustain + (self.value - c.sustain) * c.decay_coef;
                if (self.value - c.sustain).abs() < 1e-4 {
                    self.value = c.sustain;
                    self.stage = Stage::Sustain;
                }
            }
            Stage::Sustain => self.value = c.sustain,
            Stage::Release => {
                self.value *= c.release_coef;
                if self.value < 1e-4 {
                    self.value = 0.0;
                    self.stage = Stage::Idle;
                }
            }
        }
        self.value
    }
}

// ---------------------------------------------------------------------------------------------
// 2x decimator (linear-phase half-band FIR)
// ---------------------------------------------------------------------------------------------

pub const HALFBAND_TAPS: usize = 47;

/// Converts a stream running at twice the host rate back to the host rate. Used so that distortion,
/// FM and filters can run oversampled. Latency is `(HALFBAND_TAPS - 1) / 2` high-rate samples.
#[derive(Clone)]
pub struct Decimator2x {
    coef: [f32; HALFBAND_TAPS],
    hist: [f32; HALFBAND_TAPS * 2],
    pos: usize,
}

impl Default for Decimator2x {
    fn default() -> Self {
        Self::new()
    }
}

impl Decimator2x {
    pub fn new() -> Self {
        let n = HALFBAND_TAPS;
        let mid = (n - 1) as f64 / 2.0;
        let mut coef = [0.0f32; HALFBAND_TAPS];
        let mut sum = 0.0f64;
        let mut tmp = [0.0f64; HALFBAND_TAPS];
        for (k, t) in tmp.iter_mut().enumerate() {
            let m = k as f64 - mid;
            let x = m / 2.0;
            let sinc = if m == 0.0 { 1.0 } else { (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x) };
            let w = 0.42
                - 0.5 * (2.0 * std::f64::consts::PI * k as f64 / (n - 1) as f64).cos()
                + 0.08 * (4.0 * std::f64::consts::PI * k as f64 / (n - 1) as f64).cos();
            *t = 0.5 * sinc * w;
            sum += *t;
        }
        for (c, t) in coef.iter_mut().zip(tmp.iter()) {
            *c = (*t / sum) as f32;
        }
        Self { coef, hist: [0.0; HALFBAND_TAPS * 2], pos: 0 }
    }

    pub fn reset(&mut self) {
        self.hist = [0.0; HALFBAND_TAPS * 2];
        self.pos = 0;
    }

    #[inline]
    fn push(&mut self, x: f32) {
        self.pos = (self.pos + HALFBAND_TAPS - 1) % HALFBAND_TAPS;
        self.hist[self.pos] = x;
        self.hist[self.pos + HALFBAND_TAPS] = x;
    }

    /// Feed two consecutive high-rate samples, get one host-rate sample.
    #[inline]
    pub fn process(&mut self, first: f32, second: f32) -> f32 {
        self.push(first);
        self.push(second);
        let window = &self.hist[self.pos..self.pos + HALFBAND_TAPS];
        window.iter().zip(self.coef.iter()).map(|(h, c)| h * c).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimator_passes_dc_and_low_tones_and_kills_images() {
        let mut d = Decimator2x::new();
        let mut last = 0.0;
        for _ in 0..200 {
            last = d.process(1.0, 1.0);
        }
        assert!((last - 1.0).abs() < 1e-3, "DC gain {last}");

        // 1 kHz tone at a 96 kHz high rate should come through at ~unity amplitude.
        let mut d = Decimator2x::new();
        let mut peak = 0.0f32;
        for i in 0..2000 {
            let a = (std::f32::consts::TAU * 1000.0 * (2 * i) as f32 / 96_000.0).sin();
            let b = (std::f32::consts::TAU * 1000.0 * (2 * i + 1) as f32 / 96_000.0).sin();
            let y = d.process(a, b);
            if i > 200 {
                peak = peak.max(y.abs());
            }
        }
        assert!((peak - 1.0).abs() < 0.02, "1 kHz peak {peak}");

        // 35 kHz is above the host Nyquist (24 kHz) and must be strongly attenuated.
        let mut d = Decimator2x::new();
        let mut peak = 0.0f32;
        for i in 0..2000 {
            let a = (std::f32::consts::TAU * 35_000.0 * (2 * i) as f32 / 96_000.0).sin();
            let b = (std::f32::consts::TAU * 35_000.0 * (2 * i + 1) as f32 / 96_000.0).sin();
            let y = d.process(a, b);
            if i > 200 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak < 0.01, "35 kHz leaked at {peak}");
    }

    #[test]
    fn adsr_reaches_sustain_and_releases() {
        let c = AdsrCoefs::new(5.0, 50.0, 0.5, 50.0, 48_000.0);
        let mut e = Adsr::default();
        e.gate_on();
        let mut v = 0.0;
        for _ in 0..48_000 {
            v = e.next(&c);
        }
        assert!((v - 0.5).abs() < 1e-3);
        e.gate_off();
        for _ in 0..48_000 {
            e.next(&c);
        }
        assert!(e.is_idle());
    }
}
