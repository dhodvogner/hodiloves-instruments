//! Envelopes and LFOs.

use crate::params::LfoShape;
use crate::util::Rng;

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

    /// (stage, level) for the editor: 0 idle, 1 attack, 2 decay, 3 sustain, 4 release.
    pub fn view(&self) -> (u8, f32) {
        let stage = match self.stage {
            Stage::Idle => 0,
            Stage::Attack => 1,
            Stage::Decay => 2,
            Stage::Sustain => 3,
            Stage::Release => 4,
        };
        (stage, self.value)
    }

    pub fn gate_on(&mut self) {
        self.stage = Stage::Attack; // keeps `value`, so retriggers do not click
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

#[derive(Clone)]
pub struct LfoState {
    pub phase: f32,
    cur: f32,
    next: f32,
    smooth: f32,
}

impl Default for LfoState {
    fn default() -> Self {
        Self { phase: 0.0, cur: 0.0, next: 0.0, smooth: 0.0 }
    }
}

impl LfoState {
    pub fn reset(&mut self, phase: f32, rng: &mut Rng) {
        self.phase = phase;
        self.cur = rng.next_bipolar();
        self.next = rng.next_bipolar();
        self.smooth = self.cur;
    }

    /// Advance by `samples` samples and return the bipolar (-1..1) value at the end of the block.
    pub fn advance(&mut self, shape: LfoShape, inc: f32, samples: usize, rng: &mut Rng) -> f32 {
        let mut p = self.phase + inc * samples as f32;
        while p >= 1.0 {
            p -= 1.0;
            self.cur = self.next;
            self.next = rng.next_bipolar();
        }
        self.phase = p;
        match shape {
            LfoShape::Sine => (p * std::f32::consts::TAU).sin(),
            LfoShape::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
            LfoShape::SawUp => 2.0 * p - 1.0,
            LfoShape::SawDown => 1.0 - 2.0 * p,
            LfoShape::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            LfoShape::SampleHold => self.cur,
            LfoShape::SmoothRandom => {
                let t = p * p * (3.0 - 2.0 * p);
                self.smooth = self.cur + (self.next - self.cur) * t;
                self.smooth
            }
        }
    }
}

/// Pure LFO shape evaluation for UI drawing (no random shapes).
pub fn lfo_preview(shape: LfoShape, p: f32) -> f32 {
    match shape {
        LfoShape::Sine => (p * std::f32::consts::TAU).sin(),
        LfoShape::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
        LfoShape::SawUp => 2.0 * p - 1.0,
        LfoShape::SawDown => 1.0 - 2.0 * p,
        LfoShape::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoShape::SampleHold => {
            let step = (p * 6.0) as usize;
            [0.6, -0.4, 0.9, -0.8, 0.2, -0.1][step.min(5)]
        }
        LfoShape::SmoothRandom => (p * 6.28).sin() * 0.6 + (p * 15.7).sin() * 0.3,
    }
}
