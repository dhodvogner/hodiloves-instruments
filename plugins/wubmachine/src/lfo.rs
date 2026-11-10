//! The wobble LFO.

use crate::params::LfoShape;
use plugin_common::dsp::{fast_tanh, Rng};
use std::f32::consts::{PI, TAU};

/// Unipolar (0..1) value of a shape at phase `p` (0..1). `Random` is handled by the caller.
pub fn shape_value(shape: LfoShape, p: f32) -> f32 {
    match shape {
        LfoShape::Sine => 0.5 - 0.5 * (p * TAU).cos(),
        LfoShape::Triangle => {
            if p < 0.5 {
                2.0 * p
            } else {
                2.0 - 2.0 * p
            }
        }
        LfoShape::SawUp => p,
        LfoShape::SawDown => 1.0 - p,
        LfoShape::Square => {
            if p < 0.5 {
                1.0
            } else {
                0.0
            }
        }
        LfoShape::SoftSquare => 0.5 + 0.5 * fast_tanh(3.0 * (p * TAU).cos()),
        LfoShape::HalfSine => (p * PI).sin(),
        LfoShape::Random => 0.5,
    }
}

#[derive(Clone, Default)]
pub struct Lfo {
    pub phase: f32,
    held: f32,
    smooth: f32,
}

impl Lfo {
    pub fn restart(&mut self, rng: &mut Rng) {
        self.phase = 0.0;
        self.held = rng.next_f32();
        self.smooth = f32::NAN; // snaps to the first value instead of gliding from the old one
    }

    /// Advance by `cycles` (can span several wraps).
    pub fn advance(&mut self, cycles: f32, rng: &mut Rng) {
        let np = self.phase + cycles.max(0.0);
        if np >= 1.0 {
            self.held = rng.next_f32();
        }
        self.phase = np - np.floor();
    }

    /// Jump to an absolute phase (used for host-grid locking).
    pub fn set_phase(&mut self, phase: f32, rng: &mut Rng) {
        let p = phase - phase.floor();
        if p < self.phase {
            self.held = rng.next_f32();
        }
        self.phase = p;
    }

    /// Current output: 0..1, or -1..1 when `bipolar`. `block_secs` drives the smoothing filter.
    pub fn output(&mut self, shape: LfoShape, phase_offset: f32, smooth: f32, bipolar: bool, block_secs: f32) -> f32 {
        let p = {
            let q = self.phase + phase_offset;
            q - q.floor()
        };
        let raw = if shape == LfoShape::Random { self.held } else { shape_value(shape, p) };
        let tau = smooth.clamp(0.0, 1.0).powi(2) * 0.04; // up to 40 ms
        let k = if tau < 1e-5 { 1.0 } else { 1.0 - (-block_secs / tau).exp() };
        if self.smooth.is_nan() {
            self.smooth = raw;
        } else {
            self.smooth += (raw - self.smooth) * k;
        }
        if bipolar {
            self.smooth * 2.0 - 1.0
        } else {
            self.smooth
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug::prelude::Enum;

    #[test]
    fn shapes_stay_in_unit_range() {
        for k in 0..8 {
            let shape = LfoShape::from_index(k);
            for i in 0..=100 {
                let v = shape_value(shape, i as f32 / 100.0);
                assert!((-0.001..=1.001).contains(&v), "{shape:?} -> {v}");
            }
        }
    }
}
