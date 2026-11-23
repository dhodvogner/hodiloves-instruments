//! A small synthesized drum kit so you can hear the drum part: kick, snare, clap, closed and open
//! hat, two toms and a cymbal, played from General MIDI drum notes (the same notes Neon Break uses).
//! Every hit is a one-shot: it plays out on its own and ignores note-offs.

use plugin_common::dsp::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Kick,
    Snare,
    Clap,
    HatClosed,
    HatOpen,
    TomHigh,
    TomLow,
    Cymbal,
}

fn kind_for(note: u8) -> Option<Kind> {
    Some(match note {
        35 | 36 => Kind::Kick,
        38 | 40 => Kind::Snare,
        37 | 39 => Kind::Clap,
        42 | 44 => Kind::HatClosed,
        46 => Kind::HatOpen,
        41 | 43 | 45 => Kind::TomLow,
        47 | 48 | 50 => Kind::TomHigh,
        49 | 51 | 52 | 53 | 55 | 57 | 59 => Kind::Cymbal,
        _ => return None,
    })
}

/// Length of each drum in seconds (after that the voice is freed).
fn length(kind: Kind) -> f32 {
    match kind {
        Kind::Kick => 0.45,
        Kind::Snare => 0.35,
        Kind::Clap => 0.3,
        Kind::HatClosed => 0.09,
        Kind::HatOpen => 0.55,
        Kind::TomHigh | Kind::TomLow => 0.45,
        Kind::Cymbal => 1.6,
    }
}

#[derive(Clone, Copy)]
struct Hit {
    kind: Kind,
    /// Samples since the hit started.
    t: u32,
    vel: f32,
    phase: f32,
    lp: f32,
    pan: f32,
    active: bool,
}

pub struct DrumSynth {
    hits: Vec<Hit>,
    sample_rate: f32,
    rng: Rng,
}

const VOICES: usize = 24;

impl DrumSynth {
    pub fn new(sample_rate: f32) -> Self {
        let idle = Hit { kind: Kind::Kick, t: 0, vel: 0.0, phase: 0.0, lp: 0.0, pan: 0.0, active: false };
        Self { hits: vec![idle; VOICES], sample_rate, rng: Rng::new(0x9E37_79B9) }
    }

    pub fn trigger(&mut self, note: u8, velocity: u8) {
        let Some(kind) = kind_for(note) else { return };
        let pan = match kind {
            Kind::HatClosed | Kind::HatOpen => 0.25,
            Kind::TomHigh => -0.2,
            Kind::TomLow => 0.2,
            Kind::Cymbal => -0.15,
            _ => 0.0,
        };
        let hit = Hit { kind, t: 0, vel: (velocity as f32 / 127.0).powf(0.8), phase: 0.0, lp: 0.0, pan, active: true };
        // A closed hat chokes an open one, like on a real kit.
        if kind == Kind::HatClosed {
            for h in self.hits.iter_mut().filter(|h| h.active && h.kind == Kind::HatOpen) {
                h.active = false;
            }
        }
        let slot = self.hits.iter().position(|h| !h.active).or_else(|| self.hits.iter().enumerate().max_by_key(|(_, h)| h.t).map(|(i, _)| i)).unwrap_or(0);
        self.hits[slot] = hit;
    }

    pub fn is_busy(&self) -> bool {
        self.hits.iter().any(|h| h.active)
    }

    pub fn kill_all(&mut self) {
        for h in &mut self.hits {
            h.active = false;
        }
    }

    /// Add every sounding drum to the buffers.
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], gain: f32) {
        let sr = self.sample_rate;
        let rng = &mut self.rng;
        for h in self.hits.iter_mut().filter(|h| h.active) {
            let end = (length(h.kind) * sr) as u32;
            for (l, r) in out_l.iter_mut().zip(out_r.iter_mut()) {
                if h.t >= end {
                    h.active = false;
                    break;
                }
                let s = h.t as f32 / sr;
                let noise = rng.next_bipolar();
                let x = match h.kind {
                    Kind::Kick => {
                        let f = 46.0 + 120.0 * (-s * 38.0).exp();
                        h.phase = (h.phase + f / sr).fract();
                        (h.phase * std::f32::consts::TAU).sin() * (-s * 9.0).exp() * 1.0 + noise * (-s * 600.0).exp() * 0.25
                    }
                    Kind::Snare => {
                        h.phase = (h.phase + 185.0 / sr).fract();
                        let tone = (h.phase * std::f32::consts::TAU).sin() * (-s * 24.0).exp() * 0.55;
                        h.lp += 0.35 * (noise - h.lp);
                        tone + (noise - h.lp) * (-s * 17.0).exp() * 0.8
                    }
                    Kind::Clap => {
                        h.lp += 0.45 * (noise - h.lp);
                        let hp = noise - h.lp;
                        let mut env = 0.0;
                        for k in 0..3 {
                            let st = k as f32 * 0.011;
                            if s >= st {
                                env += (-(s - st) * 70.0).exp() * 0.6;
                            }
                        }
                        env += if s > 0.03 { (-(s - 0.03) * 16.0).exp() * 0.5 } else { 0.0 };
                        hp * env
                    }
                    Kind::HatClosed | Kind::HatOpen => {
                        h.lp += 0.7 * (noise - h.lp);
                        let decay = if h.kind == Kind::HatClosed { 75.0 } else { 9.0 };
                        (noise - h.lp) * (-s * decay).exp() * 0.55
                    }
                    Kind::TomHigh | Kind::TomLow => {
                        let base = if h.kind == Kind::TomHigh { 190.0 } else { 118.0 };
                        let f = base * (1.0 + 0.6 * (-s * 18.0).exp());
                        h.phase = (h.phase + f / sr).fract();
                        (h.phase * std::f32::consts::TAU).sin() * (-s * 9.0).exp() * 0.9
                    }
                    Kind::Cymbal => {
                        h.lp += 0.6 * (noise - h.lp);
                        (noise - h.lp) * (-s * 3.2).exp() * 0.4
                    }
                };
                let v = x * h.vel * gain * 0.7;
                *l += v * (1.0 - h.pan.max(0.0));
                *r += v * (1.0 + h.pan.min(0.0));
                h.t += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(note: u8, vel: u8, seconds: f32) -> Vec<f32> {
        let sr = 48_000.0;
        let mut d = DrumSynth::new(sr);
        d.trigger(note, vel);
        let n = (seconds * sr) as usize;
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        d.render(&mut l, &mut r, 1.0);
        l
    }

    fn energy(x: &[f32]) -> f32 {
        x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32
    }

    #[test]
    fn every_drum_is_audible_finite_and_decays() {
        for note in [36u8, 38, 39, 42, 46, 50, 45, 49] {
            let x = play(note, 110, 2.0);
            assert!(x.iter().all(|v| v.is_finite()), "note {note}");
            let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.05 && peak < 1.5, "note {note} peak {peak}");
            assert!(energy(&x[..4800]) > 1e-4, "note {note} silent at the start");
            assert!(energy(&x[x.len() - 4800..]) < 1e-9, "note {note} did not die");
        }
    }

    #[test]
    fn a_kick_is_low_and_a_hat_is_bright() {
        // zero crossings per second: a kick (about 50-150 Hz) crosses far less than hiss
        let crossings = |x: &[f32]| x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        let kick = play(36, 110, 0.2);
        let hat = play(42, 110, 0.08);
        assert!(crossings(&kick) < 60, "{}", crossings(&kick));
        assert!(crossings(&hat) as f32 / 0.08 > 8000.0, "{}", crossings(&hat));
    }

    #[test]
    fn velocity_changes_loudness_and_unknown_notes_are_ignored() {
        assert!(energy(&play(38, 120, 0.3)) > energy(&play(38, 30, 0.3)) * 2.0);
        let mut d = DrumSynth::new(48_000.0);
        d.trigger(10, 100);
        assert!(!d.is_busy());
    }

    #[test]
    fn closed_hat_chokes_the_open_hat() {
        let mut d = DrumSynth::new(48_000.0);
        d.trigger(46, 100);
        d.trigger(42, 100);
        assert_eq!(d.hits.iter().filter(|h| h.active).count(), 1);
    }

    #[test]
    fn many_hits_never_panic_and_free_up() {
        let mut d = DrumSynth::new(44_100.0);
        for i in 0..200u8 {
            d.trigger(35 + i % 25, 100);
        }
        let (mut l, mut r) = (vec![0.0; 44_100 * 2], vec![0.0; 44_100 * 2]); // a cymbal rings for 1.6 s
        d.render(&mut l, &mut r, 1.0);
        assert!(l.iter().all(|v| v.is_finite()));
        assert!(!d.is_busy());
    }
}
