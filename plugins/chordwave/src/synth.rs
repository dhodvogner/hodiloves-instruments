//! A small polyphonic synth for auditioning ideas without loading another instrument: pads,
//! plucks and keys for the chords, a sub bass and a lead. Not meant to replace your own synths:
//! turn on MIDI out (or export the MIDI) and play the parts on those.

use plugin_common::dsp::{midi_to_hz, Adsr, AdsrCoefs, Ladder};

pub const PARTS: usize = 3;
pub const PART_CHORDS: u8 = 0;
pub const PART_BASS: u8 = 1;
pub const PART_MELODY: u8 = 2;

pub const SOUND_NAMES: [&str; 3] = ["Pad", "Pluck", "Keys"];

#[derive(Clone, Copy)]
pub struct Patch {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub cutoff: f32,
    pub resonance: f32,
    /// How far the envelope opens the filter (multiples of the base cutoff).
    pub env_amount: f32,
    pub detune_cents: f32,
    pub sub: f32,
    pub saw: f32,
    pub level: f32,
}

pub fn patch(part: u8, sound: usize) -> Patch {
    match part {
        PART_BASS => Patch { attack: 3.0, decay: 120.0, sustain: 0.9, release: 90.0, cutoff: 380.0, resonance: 1.1,
            env_amount: 1.5, detune_cents: 0.0, sub: 0.9, saw: 0.45, level: 0.5 },
        PART_MELODY => Patch { attack: 6.0, decay: 220.0, sustain: 0.7, release: 160.0, cutoff: 3200.0, resonance: 0.8,
            env_amount: 1.0, detune_cents: 7.0, sub: 0.0, saw: 0.8, level: 0.2 },
        _ => match sound {
            1 => Patch { attack: 2.0, decay: 260.0, sustain: 0.0, release: 140.0, cutoff: 1300.0, resonance: 1.0,
                env_amount: 3.0, detune_cents: 6.0, sub: 0.0, saw: 0.8, level: 0.2 },
            2 => Patch { attack: 3.0, decay: 800.0, sustain: 0.3, release: 260.0, cutoff: 2800.0, resonance: 0.3,
                env_amount: 1.2, detune_cents: 3.0, sub: 0.15, saw: 0.55, level: 0.2 },
            _ => Patch { attack: 45.0, decay: 400.0, sustain: 0.8, release: 520.0, cutoff: 2000.0, resonance: 0.6,
                env_amount: 0.6, detune_cents: 10.0, sub: 0.0, saw: 0.85, level: 0.16 },
        },
    }
}

#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[inline]
fn saw(phase: f32, dt: f32) -> f32 {
    2.0 * phase - 1.0 - poly_blep(phase, dt)
}

#[derive(Clone)]
pub struct Voice {
    pub pitch: u8,
    pub part: u8,
    pub gated: bool,
    vel: f32,
    phase: [f32; 3],
    env: Adsr,
    coefs: AdsrCoefs,
    patch: Patch,
    filter: Ladder,
    /// Samples since the note started, used to steal the oldest voice.
    age: u64,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            pitch: 0,
            part: 0,
            gated: false,
            vel: 0.0,
            phase: [0.0; 3],
            env: Adsr::default(),
            coefs: AdsrCoefs::default(),
            patch: patch(0, 0),
            filter: Ladder::default(),
            age: 0,
        }
    }
}

impl Voice {
    pub fn is_idle(&self) -> bool {
        self.env.is_idle()
    }

    pub fn start(&mut self, pitch: u8, part: u8, sound: usize, velocity: u8, sample_rate: f32) {
        self.pitch = pitch;
        self.part = part;
        self.vel = velocity as f32 / 127.0;
        self.patch = patch(part, sound);
        let p = &self.patch;
        self.coefs = AdsrCoefs::new(p.attack, p.decay, p.sustain, p.release, sample_rate);
        // Start the oscillators at slightly different phases so unison does not sum into a click.
        self.phase = [0.0, 0.31, 0.62];
        self.filter.reset();
        self.env.kill();
        self.env.gate_on();
        self.gated = true;
        self.age = 0;
    }

    pub fn release(&mut self) {
        self.gated = false;
        self.env.gate_off();
    }

    pub fn kill(&mut self) {
        self.gated = false;
        self.env.kill();
    }

    pub fn age(&self) -> u64 {
        self.age
    }

    /// Add this voice's next sample to a stereo frame.
    #[inline]
    pub fn tick(&mut self, sample_rate: f32) -> (f32, f32) {
        let env = self.env.next(&self.coefs);
        if self.env.is_idle() {
            return (0.0, 0.0);
        }
        self.age += 1;
        let p = &self.patch;
        let hz = midi_to_hz(self.pitch as f32);
        let detune = (p.detune_cents / 1200.0).exp2();
        let (f1, f2) = (hz / detune, hz * detune);
        let (d1, d2, d3) = (f1 / sample_rate, f2 / sample_rate, hz * 0.5 / sample_rate);
        let a = saw(self.phase[0], d1);
        let b = saw(self.phase[1], d2);
        let sub = (self.phase[2] * std::f32::consts::TAU).sin();
        for (ph, d) in self.phase.iter_mut().zip([d1, d2, d3]) {
            *ph += d;
            if *ph >= 1.0 {
                *ph -= 1.0;
            }
        }
        // Stereo: the two detuned saws lean left and right, the sub stays in the middle.
        let l = a * 0.75 + b * 0.25;
        let r = a * 0.25 + b * 0.75;
        let mid = sub * p.sub;
        let (l, r) = (l * p.saw + mid, r * p.saw + mid);

        let cutoff = (p.cutoff * (1.0 + p.env_amount * env * (0.5 + 0.5 * self.vel))).clamp(40.0, sample_rate * 0.42);
        let g = (std::f32::consts::PI * cutoff / sample_rate).tan();
        // Filter the mid signal once and apply to both channels: cheap, and stereo comes from the oscillators.
        let mono = self.filter.process((l + r) * 0.5, g, p.resonance);
        let side = (l - r) * 0.5;
        let amp = env * p.level * (0.35 + 0.65 * self.vel);
        ((mono + side * 0.7) * amp, (mono - side * 0.7) * amp)
    }
}

pub const VOICES: usize = 40;

pub struct Synth {
    pub voices: Vec<Voice>,
    pub sample_rate: f32,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self { voices: vec![Voice::default(); VOICES], sample_rate }
    }

    pub fn note_on(&mut self, pitch: u8, part: u8, sound: usize, velocity: u8) {
        let sr = self.sample_rate;
        // Retrigger the same note of the same part, else take a free voice, else the oldest.
        let index = self
            .voices
            .iter()
            .position(|v| v.gated && v.pitch == pitch && v.part == part)
            .or_else(|| self.voices.iter().position(|v| v.is_idle()))
            .or_else(|| self.voices.iter().enumerate().max_by_key(|(_, v)| v.age()).map(|(i, _)| i))
            .unwrap_or(0);
        self.voices[index].start(pitch, part, sound, velocity, sr);
    }

    pub fn note_off(&mut self, pitch: u8, part: u8) {
        for v in self.voices.iter_mut().filter(|v| v.gated && v.pitch == pitch && v.part == part) {
            v.release();
        }
    }

    pub fn all_off(&mut self) {
        for v in &mut self.voices {
            v.release();
        }
    }

    pub fn kill_all(&mut self) {
        for v in &mut self.voices {
            v.kill();
        }
    }

    pub fn is_busy(&self) -> bool {
        self.voices.iter().any(|v| !v.is_idle())
    }

    /// Mix `out_l` / `out_r` with every voice (adds to what is already there).
    pub fn render(&mut self, out_l: &mut [f32], out_r: &mut [f32], part_gain: [f32; PARTS]) {
        let sr = self.sample_rate;
        for v in self.voices.iter_mut().filter(|v| !v.is_idle()) {
            let gain = part_gain[(v.part as usize).min(PARTS - 1)];
            for (l, r) in out_l.iter_mut().zip(out_r.iter_mut()) {
                let (a, b) = v.tick(sr);
                *l += a * gain;
                *r += b * gain;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_makes_sound_and_dies_after_release() {
        let mut s = Synth::new(48_000.0);
        s.note_on(57, PART_CHORDS, 0, 100);
        let (mut l, mut r) = (vec![0.0; 4800], vec![0.0; 4800]);
        s.render(&mut l, &mut r, [1.0; PARTS]);
        let peak = l.iter().chain(&r).fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.02 && peak < 1.0, "peak {peak}");
        assert!(l.iter().all(|x| x.is_finite()));
        s.note_off(57, PART_CHORDS);
        for _ in 0..20 {
            l.fill(0.0);
            r.fill(0.0);
            s.render(&mut l, &mut r, [1.0; PARTS]);
        }
        assert!(!s.is_busy());
    }

    #[test]
    fn every_patch_is_audible_and_stable() {
        for part in 0..PARTS as u8 {
            for sound in 0..SOUND_NAMES.len() {
                let mut s = Synth::new(44_100.0);
                for n in [36u8, 60, 84] {
                    s.note_on(n, part, sound, 110);
                }
                let (mut l, mut r) = (vec![0.0; 2048], vec![0.0; 2048]);
                s.render(&mut l, &mut r, [1.0; PARTS]);
                let peak = l.iter().chain(&r).fold(0.0f32, |m, x| m.max(x.abs()));
                assert!(peak > 0.005 && peak < 2.0 && l.iter().all(|x| x.is_finite()), "part {part} sound {sound} peak {peak}");
            }
        }
    }

    #[test]
    fn voice_stealing_never_panics() {
        let mut s = Synth::new(44_100.0);
        for n in 0..100u8 {
            s.note_on(30 + n % 60, PART_CHORDS, 0, 90);
        }
        assert!(s.is_busy());
    }
}
