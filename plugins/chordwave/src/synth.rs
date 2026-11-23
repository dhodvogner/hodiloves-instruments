//! A small polyphonic synth for auditioning ideas without loading another instrument: pads,
//! plucks and keys for the chords, a sub bass and a lead. Not meant to replace your own synths:
//! turn on MIDI out (or export the MIDI) and play the parts on those.

use crate::drumsynth::DrumSynth;
use plugin_common::dsp::{midi_to_hz, Adsr, AdsrCoefs, Ladder};

pub const PARTS: usize = 4;
pub const PART_CHORDS: u8 = 0;
pub const PART_BASS: u8 = 1;
pub const PART_MELODY: u8 = 2;
pub const PART_DRUMS: u8 = 3;

pub const SOUND_NAMES: [&str; 5] = ["Pad", "Pluck", "Keys", "Strings", "Bright"];
pub const BASS_SOUND_NAMES: [&str; 3] = ["Sub", "Saw", "Reese"];
pub const MELODY_SOUND_NAMES: [&str; 4] = ["Lead", "Pluck", "Soft", "Bright"];

/// The sounds of one part (0 chords, 1 bass, 2 melody) with a plain-words description each.
pub fn sounds_for(part: u8) -> (&'static [&'static str], &'static [&'static str]) {
    match part {
        PART_BASS => (&BASS_SOUND_NAMES, &[
            "A deep, clean low sine with a little edge. Solid and modern: works under almost anything.",
            "A buzzy sawtooth bass with a filter that opens on each note. More aggressive and audible on small speakers.",
            "Several detuned saws, slowly shifting against each other: the wide, growling Reese bass of drum and bass.",
        ]),
        PART_MELODY => (&MELODY_SOUND_NAMES, &[
            "A classic detuned saw lead: bright, bold and easy to hear over the chords.",
            "A short, plucky note that dies away quickly: playful, great for arpeggio-style melodies.",
            "A mellow, rounded tone with a slow attack: gentle and emotional.",
            "A big, wide, super-saw style lead: shiny and euphoric (trance, synthwave anthems).",
        ]),
        _ => (&SOUND_NAMES, &[
            "A warm, slow-fading synth pad with a gentle filter. The default sound of this plugin.",
            "Short, plucky chords that decay fast. Clear and rhythmic, good for stabs and arpeggios.",
            "A soft electric-piano-like tone. Relaxed, a bit jazzy.",
            "A slow, wide, string-like pad with a long release. Cinematic and emotional.",
            "A bright, wide super-saw chord sound with a fast attack. Energetic and anthemic.",
        ]),
    }
}

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

#[allow(clippy::too_many_arguments)]
const fn pt(attack: f32, decay: f32, sustain: f32, release: f32, cutoff: f32, resonance: f32, env_amount: f32, detune_cents: f32, sub: f32, saw: f32, level: f32) -> Patch {
    Patch { attack, decay, sustain, release, cutoff, resonance, env_amount, detune_cents, sub, saw, level }
}

pub fn patch(part: u8, sound: usize) -> Patch {
    match part {
        PART_BASS => match sound {
            1 => pt(3.0, 160.0, 0.8, 90.0, 900.0, 1.4, 2.5, 3.0, 0.3, 0.9, 0.42),
            2 => pt(12.0, 200.0, 0.9, 140.0, 520.0, 0.8, 1.0, 14.0, 0.5, 0.9, 0.42),
            _ => pt(3.0, 120.0, 0.9, 90.0, 380.0, 1.1, 1.5, 0.0, 0.9, 0.45, 0.5),
        },
        PART_MELODY => match sound {
            1 => pt(2.0, 220.0, 0.0, 120.0, 2000.0, 0.8, 3.0, 5.0, 0.0, 0.8, 0.2),
            2 => pt(25.0, 300.0, 0.8, 300.0, 1500.0, 0.2, 0.4, 4.0, 0.0, 0.4, 0.22),
            3 => pt(5.0, 300.0, 0.75, 220.0, 6000.0, 0.3, 0.8, 20.0, 0.0, 0.95, 0.15),
            _ => pt(6.0, 220.0, 0.7, 160.0, 3200.0, 0.8, 1.0, 7.0, 0.0, 0.8, 0.2),
        },
        _ => match sound {
            1 => pt(2.0, 260.0, 0.0, 140.0, 1300.0, 1.0, 3.0, 6.0, 0.0, 0.8, 0.2),
            2 => pt(3.0, 800.0, 0.3, 260.0, 2800.0, 0.3, 1.2, 3.0, 0.15, 0.55, 0.2),
            3 => pt(140.0, 500.0, 0.85, 800.0, 2600.0, 0.4, 0.3, 14.0, 0.0, 0.95, 0.15),
            4 => pt(8.0, 300.0, 0.75, 300.0, 5200.0, 0.3, 0.8, 22.0, 0.0, 0.95, 0.15),
            _ => pt(45.0, 400.0, 0.8, 520.0, 2000.0, 0.6, 0.6, 10.0, 0.0, 0.85, 0.16),
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
    pub drums: DrumSynth,
    pub sample_rate: f32,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self { voices: vec![Voice::default(); VOICES], drums: DrumSynth::new(sample_rate), sample_rate }
    }

    pub fn note_on(&mut self, pitch: u8, part: u8, sound: usize, velocity: u8) {
        if part == PART_DRUMS {
            self.drums.trigger(pitch, velocity);
            return;
        }
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
        if part == PART_DRUMS {
            return; // drum hits play out on their own
        }
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
        self.drums.kill_all();
    }

    pub fn is_busy(&self) -> bool {
        self.voices.iter().any(|v| !v.is_idle()) || self.drums.is_busy()
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
        self.drums.render(out_l, out_r, part_gain[PART_DRUMS as usize]);
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
        for part in 0..3u8 {
            for sound in 0..sounds_for(part).0.len() {
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
    fn every_sound_has_a_name_and_a_description() {
        for part in 0..3u8 {
            let (names, blurbs) = sounds_for(part);
            assert_eq!(names.len(), blurbs.len(), "part {part}");
            assert!(names.len() >= 3);
        }
    }

    #[test]
    fn drums_play_through_the_synth_and_ignore_note_off() {
        let mut s = Synth::new(48_000.0);
        s.note_on(36, PART_DRUMS, 0, 110);
        s.note_off(36, PART_DRUMS);
        let (mut l, mut r) = (vec![0.0; 2400], vec![0.0; 2400]);
        s.render(&mut l, &mut r, [1.0; PARTS]);
        assert!(l.iter().fold(0.0f32, |m, x| m.max(x.abs())) > 0.05);
        // muted by its gain
        let mut s = Synth::new(48_000.0);
        s.note_on(36, PART_DRUMS, 0, 110);
        let (mut l, mut r) = (vec![0.0; 2400], vec![0.0; 2400]);
        s.render(&mut l, &mut r, [1.0, 1.0, 1.0, 0.0]);
        assert!(l.iter().all(|x| *x == 0.0));
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
