//! The eight synthesized drums. No samples: every hit is built from sine oscillators with pitch
//! sweeps, clusters of square waves and filtered noise, the way analog and early digital drum
//! machines did it.

use plugin_common::dsp::{fast_tanh, lerp, Rng};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Kick,
    Snare,
    Clap,
    HatClosed,
    HatOpen,
    TomHi,
    TomLo,
    Cymbal,
}

pub const KINDS: [Kind; 8] = [
    Kind::Kick,
    Kind::Snare,
    Kind::Clap,
    Kind::HatClosed,
    Kind::HatOpen,
    Kind::TomHi,
    Kind::TomLo,
    Kind::Cymbal,
];

/// The sound-shaping knobs of one drum, read when a hit starts (so turning a knob never zippers a
/// ringing hit; the next hit simply uses the new value).
#[derive(Clone, Copy, Debug, Default)]
pub struct Shape {
    /// Semitones.
    pub tune: f32,
    pub decay: f32,
    pub tone: f32,
    pub a: f32,
    pub b: f32,
}

/// Trapezoidal state-variable filter; returns (low, band, high).
#[derive(Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `g` = tan(pi * cutoff / sample_rate), `k` = 1 / Q.
    #[inline]
    pub fn process(&mut self, x: f32, g: f32, k: f32) -> (f32, f32, f32) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - k * v1 - v2)
    }
}

/// Square-wave frequencies of the metallic cluster (the 808 cymbal ratios, plus two for the crash).
const METAL_HZ: [f32; 8] = [205.3, 304.4, 369.6, 522.7, 540.0, 800.0, 1066.0, 1432.0];

pub struct Drum {
    kind: Kind,
    sr: f32,
    inv_sr: f32,
    rng: Rng,
    on: bool,
    vel: f32,
    /// Samples since the hit started.
    n: u32,
    ph: [f32; 8],
    inc: [f32; 8],
    /// Main and secondary amplitude envelopes and the pitch envelope, with their per-sample decay.
    amp: f32,
    amp2: f32,
    pitch: f32,
    k_amp: f32,
    k_amp2: f32,
    k_pitch: f32,
    f0: f32,
    depth: f32,
    /// Filter coefficients.
    g: f32,
    kq: f32,
    svf: Svf,
    shape: Shape,
    /// Clap: sample index at which the next of the early bursts starts, and how many are left.
    next_burst: u32,
    bursts_left: u8,
    burst_gap: u32,
    k_tail: f32,
    /// Last output and the fading remainder of an interrupted hit, so retriggers do not click.
    last: f32,
    declick: f32,
    k_declick: f32,
}

impl Drum {
    pub fn new(kind: Kind, sample_rate: f32, seed: u32) -> Self {
        Self {
            kind,
            sr: sample_rate,
            inv_sr: 1.0 / sample_rate,
            rng: Rng::new(seed),
            on: false,
            vel: 0.0,
            n: 0,
            ph: [0.0; 8],
            inc: [0.0; 8],
            amp: 0.0,
            amp2: 0.0,
            pitch: 0.0,
            k_amp: 0.0,
            k_amp2: 0.0,
            k_pitch: 0.0,
            f0: 100.0,
            depth: 0.0,
            g: 0.1,
            kq: 1.0,
            svf: Svf::default(),
            shape: Shape::default(),
            next_burst: 0,
            bursts_left: 0,
            burst_gap: 1,
            k_tail: 0.0,
            last: 0.0,
            declick: 0.0,
            k_declick: (-1.0 / (0.0015 * sample_rate)).exp(),
        }
    }

    pub fn reset(&mut self) {
        self.on = false;
        self.last = 0.0;
        self.declick = 0.0;
        self.svf.reset();
    }

    pub fn is_on(&self) -> bool {
        self.on || self.declick.abs() > 1e-5
    }

    /// Per-sample multiplier of an exponential that falls to 1/e in `seconds`.
    fn coef(&self, seconds: f32) -> f32 {
        (-1.0 / (seconds.max(0.0005) * self.sr)).exp()
    }

    fn filter_g(&self, hz: f32) -> f32 {
        (PI * hz.clamp(20.0, 0.45 * self.sr) * self.inv_sr).tan()
    }

    /// Cut a ringing hit short (the closed hat chokes the open hat).
    pub fn choke(&mut self) {
        if self.on {
            let fast = self.coef(0.006);
            self.k_amp = self.k_amp.min(fast);
            self.k_amp2 = self.k_amp2.min(fast);
            self.k_tail = self.k_tail.min(fast);
        }
    }

    /// Start a hit. `var` (-1..1) is a small random offset that makes repeated hits differ
    /// slightly in pitch, like a real drum that is never struck in exactly the same spot.
    pub fn trigger(&mut self, vel: f32, shape: Shape, var: f32) {
        // Whatever was still ringing fades out over about a millisecond instead of jumping to zero.
        self.declick += self.last;
        self.on = true;
        self.n = 0;
        self.vel = vel.clamp(0.0, 1.0);
        self.shape = shape;
        self.ph = [0.0; 8];
        self.svf.reset();
        self.pitch = 1.0;
        self.amp = 1.0;
        self.amp2 = 1.0;
        let mut ratio = (shape.tune / 12.0).exp2();
        if !matches!(self.kind, Kind::Kick | Kind::TomHi | Kind::TomLo) {
            ratio *= 1.0 + 0.015 * var.clamp(-1.0, 1.0);
        }
        let d2 = shape.decay * shape.decay;
        // Ghost notes are not just quieter: a soft hit is also shorter and duller.
        let soft = 0.55 + 0.45 * self.vel;
        match self.kind {
            Kind::Kick => {
                self.f0 = 47.0 * ratio;
                self.depth = 1.0 + 9.0 * shape.a;
                self.k_pitch = self.coef(0.018);
                // Starting above 1 and clamping gives a short flat "hold" before the decay.
                self.amp = 1.6;
                self.k_amp = self.coef(0.06 + 0.5 * d2);
                self.k_amp2 = self.coef(0.003);
                self.g = self.filter_g(2500.0);
                self.kq = 1.2;
            }
            Kind::Snare => {
                self.f0 = 185.0 * ratio;
                self.depth = 0.25 + 1.5 * shape.b;
                self.k_pitch = self.coef(0.025);
                self.k_amp = self.coef((0.04 + 0.14 * shape.decay) * soft);
                self.k_amp2 = self.coef((0.03 + 0.28 * d2) * soft);
                self.g = self.filter_g((900.0 + 5000.0 * shape.tone) * (0.65 + 0.35 * self.vel));
                self.kq = 1.2;
            }
            Kind::Clap => {
                self.burst_gap = ((0.007 + 0.012 * shape.a) * self.sr) as u32;
                self.next_burst = self.burst_gap;
                self.bursts_left = 3;
                self.k_amp = self.coef(0.0035);
                self.k_tail = self.coef(0.03 + 0.3 * d2);
                self.g = self.filter_g((900.0 + 2200.0 * shape.tone) * ratio);
                self.kq = 1.0 / (1.5 + 5.0 * shape.b);
            }
            Kind::HatClosed | Kind::HatOpen | Kind::Cymbal => {
                for (inc, hz) in self.inc.iter_mut().zip(METAL_HZ.iter()) {
                    *inc = hz * 2.0 * ratio * self.inv_sr;
                }
                let (tau, low) = match self.kind {
                    Kind::HatClosed => (0.012 + 0.08 * shape.decay, 6000.0),
                    Kind::HatOpen => (0.08 + 0.6 * shape.decay, 6000.0),
                    _ => (0.15 + 1.2 * shape.decay, 3500.0),
                };
                let tau = tau * soft;
                self.k_amp = self.coef(tau);
                self.k_amp2 = self.coef(0.4 * tau);
                self.g = self.filter_g(low + 5000.0 * shape.tone);
                self.kq = if self.kind == Kind::Cymbal { 1.0 } else { 1.0 / (0.7 + 3.0 * shape.b) };
                self.f0 = 587.0 * ratio;
            }
            Kind::TomHi | Kind::TomLo => {
                // Tuned like floor and rack toms; a small, quick pitch drop unless BEND is raised.
                self.f0 = if self.kind == Kind::TomHi { 128.0 } else { 84.0 } * ratio;
                self.depth = 0.06 + 0.9 * shape.a * shape.a;
                self.k_pitch = self.coef(0.03);
                self.amp = 1.3;
                self.k_amp = self.coef(0.07 + 0.45 * d2);
                self.k_amp2 = self.coef(0.03);
                // The noise is band-limited around the shell's "knock" rather than bright hiss.
                self.g = self.filter_g(if self.kind == Kind::TomHi { 1400.0 } else { 900.0 });
                self.kq = 0.8;
            }
        }
    }

    #[inline]
    fn sine(&mut self, i: usize, hz: f32) -> f32 {
        let s = (TAU * self.ph[i]).sin();
        self.ph[i] += hz * self.inv_sr;
        if self.ph[i] >= 1.0 {
            self.ph[i] -= 1.0;
        }
        s
    }

    /// Sum of the first `count` square waves of the metallic cluster, about -1..1.
    #[inline]
    fn metal(&mut self, count: usize) -> f32 {
        let mut s = 0.0;
        for i in 0..count {
            self.ph[i] += self.inc[i];
            if self.ph[i] >= 1.0 {
                self.ph[i] -= 1.0;
            }
            s += if self.ph[i] < 0.5 { 1.0 } else { -1.0 };
        }
        s / count as f32
    }

    #[inline]
    pub fn process(&mut self) -> f32 {
        let tail = self.declick;
        if tail != 0.0 {
            self.declick = if tail.abs() < 1e-5 { 0.0 } else { tail * self.k_declick };
        }
        if !self.on {
            self.last = 0.0;
            return tail;
        }
        let s = self.shape;
        let out = match self.kind {
            Kind::Kick => {
                let hz = self.f0 * (1.0 + self.depth * self.pitch);
                self.pitch *= self.k_pitch;
                let drive = 1.0 + 5.0 * s.tone;
                let body = fast_tanh(self.sine(0, hz) * drive) / fast_tanh(drive);
                let (_, _, hp) = self.svf.process(self.rng.next_bipolar(), self.g, self.kq);
                let click = hp * self.amp2 * s.b * 0.5;
                self.amp2 *= self.k_amp2;
                let y = body * self.amp.min(1.0) + click;
                self.amp *= self.k_amp;
                y
            }
            Kind::Snare => {
                let hz = self.f0 * (1.0 + self.depth * self.pitch);
                self.pitch *= self.k_pitch;
                let body = (self.sine(0, hz) * 0.7 + self.sine(1, hz * 1.59) * 0.35) * self.amp;
                let (_, bp, hp) = self.svf.process(self.rng.next_bipolar(), self.g, self.kq);
                let noise = (hp + 0.6 * bp) * self.amp2;
                self.amp *= self.k_amp;
                self.amp2 *= self.k_amp2;
                body * (1.0 - 0.6 * s.a) + noise * (0.3 + 0.8 * s.a)
            }
            Kind::Clap => {
                if self.bursts_left > 0 && self.n >= self.next_burst {
                    self.bursts_left -= 1;
                    self.next_burst += self.burst_gap;
                    self.amp = 1.0;
                    if self.bursts_left == 0 {
                        self.k_amp = self.k_tail;
                    }
                }
                let (_, bp, _) = self.svf.process(self.rng.next_bipolar(), self.g, self.kq);
                // A narrower band passes less noise, so it gets more gain.
                let y = bp * self.amp * (0.6 + 0.12 / self.kq.max(0.05)).min(2.0);
                self.amp *= self.k_amp;
                y
            }
            Kind::HatClosed | Kind::HatOpen => {
                let src = lerp(self.rng.next_bipolar(), self.metal(6) * 1.6, s.a);
                let (_, _, hp) = self.svf.process(src, self.g, self.kq);
                let y = hp * self.amp;
                self.amp *= self.k_amp;
                y
            }
            Kind::Cymbal => {
                let src = lerp(self.rng.next_bipolar(), self.metal(8) * 1.6, s.a);
                let (_, bp, hp) = self.svf.process(src, self.g, self.kq);
                // A bright burst at the start (the stick), then the long wash.
                let wash = (hp + 0.3 * bp) * (0.75 * self.amp + 0.5 * self.amp2 * self.amp2);
                // The bell partials reuse two phase slots the cluster does not need for timing.
                let f = self.f0;
                let bell = ((TAU * self.n as f32 * f * self.inv_sr).sin()
                    + 0.6 * (TAU * self.n as f32 * f * 1.48 * self.inv_sr).sin())
                    * self.amp2
                    * s.b
                    * 0.35;
                self.amp *= self.k_amp;
                self.amp2 *= self.k_amp2;
                wash + bell
            }
            Kind::TomHi | Kind::TomLo => {
                let hz = self.f0 * (1.0 + self.depth * self.pitch);
                self.pitch *= self.k_pitch;
                let drive = 1.0 + 4.0 * s.tone;
                // A second, faster-dying partial (a drum head's 1.5x overtone) makes it less of a
                // pure "synth" tone.
                let tone = self.sine(0, hz) + 0.3 * self.amp2 * self.sine(1, hz * 1.5);
                let body = fast_tanh(tone * drive) / fast_tanh(drive);
                let (_, bp, _) = self.svf.process(self.rng.next_bipolar(), self.g, self.kq);
                let stick = bp * self.amp2 * s.b * 0.9;
                self.amp2 *= self.k_amp2;
                let y = body * self.amp.min(1.0) + stick;
                self.amp *= self.k_amp;
                y
            }
        };
        self.n += 1;
        if self.amp.max(if self.has_second_envelope() { self.amp2 } else { 0.0 }) < 1e-4 {
            self.on = false;
        }
        let y = out * self.vel * self.trim();
        self.last = y;
        y + tail
    }

    /// Does the hit keep sounding through `amp2` after `amp` has died away?
    fn has_second_envelope(&self) -> bool {
        self.kind == Kind::Snare
    }

    /// Per-drum output trim so that all eight sit at a similar level with their knobs centred.
    fn trim(&self) -> f32 {
        match self.kind {
            Kind::Kick => 1.0,
            Kind::Snare => 0.9,
            Kind::Clap => 0.8,
            Kind::HatClosed => 0.55,
            Kind::HatOpen => 0.5,
            Kind::TomHi | Kind::TomLo => 0.9,
            Kind::Cymbal => 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(kind: Kind, shape: Shape, secs: f32) -> Vec<f32> {
        let sr = 48_000.0;
        let mut d = Drum::new(kind, sr, 7);
        d.trigger(1.0, shape, 0.0);
        (0..(secs * sr) as usize).map(|_| d.process()).collect()
    }

    fn centred() -> Shape {
        Shape { tune: 0.0, decay: 0.5, tone: 0.5, a: 0.5, b: 0.5 }
    }

    #[test]
    fn every_drum_makes_a_finite_sound_that_dies_away() {
        for kind in KINDS {
            let out = render(kind, centred(), 6.0);
            assert!(out.iter().all(|x| x.is_finite()), "{kind:?} produced NaN");
            let peak = out.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(peak > 0.15 && peak < 2.0, "{kind:?} peak {peak}");
            let tail = out[out.len() - 4800..].iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(tail < 1e-3, "{kind:?} still at {tail} after six seconds");
        }
    }

    #[test]
    fn decay_knob_lengthens_the_hit() {
        for kind in KINDS {
            let energy = |decay: f32| {
                let out = render(kind, Shape { decay, ..centred() }, 3.0);
                out.iter().map(|x| x * x).sum::<f32>()
            };
            assert!(energy(0.9) > 1.3 * energy(0.1), "{kind:?}: decay has too little effect");
        }
    }

    #[test]
    fn kick_is_mostly_low_and_retrigger_does_not_jump() {
        let out = render(Kind::Kick, Shape { b: 0.0, ..centred() }, 0.5);
        // Count zero crossings after the pitch sweep: about 47 Hz means few crossings.
        let crossings = out[4800..].windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert!(crossings < 40, "kick tail has {crossings} cycles in 0.4 s");

        let sr = 48_000.0;
        let mut d = Drum::new(Kind::Kick, sr, 3);
        d.trigger(1.0, Shape { b: 0.0, ..centred() }, 0.0);
        let mut prev = 0.0;
        let mut worst = 0.0f32;
        for i in 0..9000 {
            if i == 4000 {
                d.trigger(1.0, Shape { b: 0.0, ..centred() }, 0.0);
            }
            let y = d.process();
            worst = worst.max((y - prev).abs());
            prev = y;
        }
        assert!(worst < 0.2, "retrigger jumped by {worst}");
    }

    #[test]
    fn ghost_hits_are_shorter_and_duller_than_full_hits() {
        let sr = 48_000.0;
        let render = |vel: f32| {
            let mut d = Drum::new(Kind::Snare, sr, 11);
            d.trigger(vel, centred(), 0.0);
            (0..24_000).map(|_| d.process() / vel).collect::<Vec<f32>>()
        };
        // Compare the shapes with the level difference removed.
        let late = |out: &[f32]| out[4800..].iter().map(|x| x * x).sum::<f32>() / out.iter().map(|x| x * x).sum::<f32>();
        let (ghost, full) = (render(0.3), render(1.0));
        assert!(late(&ghost) < 0.7 * late(&full), "ghost {} vs full {}", late(&ghost), late(&full));
    }

    #[test]
    fn choke_silences_the_open_hat_quickly() {
        let sr = 48_000.0;
        let mut d = Drum::new(Kind::HatOpen, sr, 5);
        d.trigger(1.0, Shape { decay: 1.0, ..centred() }, 0.0);
        for _ in 0..2400 {
            d.process();
        }
        d.choke();
        let mut peak = 0.0f32;
        for i in 0..4800 {
            let y = d.process();
            if i > 2400 {
                peak = peak.max(y.abs());
            }
        }
        assert!(peak < 0.01, "choked hat still at {peak}");
    }
}
