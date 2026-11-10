//! Waveshapers for the two distortion stages. `drive` is 0..1.

use crate::params::DistKind;
use plugin_common::dsp::{fast_tanh, lerp};

#[inline]
pub fn shape(kind: DistKind, x: f32, drive: f32) -> f32 {
    let drive = drive.clamp(0.0, 1.0);
    let g = 1.0 + drive * drive * 40.0;
    match kind {
        DistKind::Off => x,
        DistKind::Soft => fast_tanh(x * g),
        DistKind::Hard => (x * g).clamp(-1.0, 1.0),
        DistKind::Fold => {
            let u = (x * g + 1.0).rem_euclid(4.0);
            1.0 - (u - 2.0).abs()
        }
        DistKind::Tube => {
            // Biased saturation: asymmetric clipping adds even harmonics (the DC is removed later).
            let v = x * g;
            fast_tanh(v + 0.3) - fast_tanh(0.3)
        }
        DistKind::Rectify => {
            let s = fast_tanh(x * g);
            lerp(s, 2.0 * s.abs() - 0.5, drive * 0.8)
        }
        DistKind::Crush => {
            let bits = 12.0 - drive * 9.0;
            let levels = (bits - 1.0).exp2();
            ((x * (1.0 + drive * 2.0) * levels).round() / levels).clamp(-4.0, 4.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_is_finite_and_bounded() {
        use nih_plug::prelude::Enum;
        for k in 0..7 {
            let kind = DistKind::from_index(k);
            for d in [0.0, 0.3, 0.7, 1.0] {
                for i in -40..=40 {
                    let x = i as f32 * 0.1;
                    let y = shape(kind, x, d);
                    assert!(y.is_finite() && y.abs() < 12.0, "{kind:?} d={d} x={x} -> {y}");
                }
            }
        }
    }
}
