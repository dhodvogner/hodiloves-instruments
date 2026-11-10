//! The drum machine itself: pattern player, the eight drums (synth and sample), gated reverb and
//! drum bus.
//!
//! Everything can be driven from the piano roll. Three groups of keys do different jobs:
//! drum notes (General MIDI, from 36) play single hits, the fill keys (60, 62, 64, 65) play a
//! fill while held, and the twelve pattern keys (72-83) play a pattern slot while held.

use crate::bank::{Grid, SLOTS};
use crate::drums::{Drum, Shape, KINDS};
use crate::fx::{Bus, Gate, Reverb};
use crate::params::{self, Fill, KickOut, NeonParams, SeqMode, STEPS, VOICES, VOICE_INFO};
use crate::sample::{Player, Sample};
use crate::seq::{self, fill_hit, HAT_C, HAT_O, KICK};
use nih_plug::prelude::Enum;
use plugin_common::dsp::Rng;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------
// Parameter snapshot
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct VoiceSnap {
    pub level: f32,
    pub pan: f32,
    pub shape: Shape,
    pub send: f32,
    pub mute: bool,
    pub sample_mix: f32,
}

/// Plain copy of every parameter the engine needs, read once per process call.
#[derive(Clone, Copy, Debug)]
pub struct Snap {
    pub voices: [VoiceSnap; VOICES],
    pub mode: SeqMode,
    /// Selected pattern slot, counted from 0.
    pub slot: usize,
    pub kick_out: KickOut,
    pub swing: f32,
    pub human: f32,
    pub fill: Fill,
    pub bpm: f32,
    pub rev_size: f32,
    pub rev_gate: params::Gate,
    pub rev_tone: f32,
    pub rev_level: f32,
    pub punch: f32,
    pub drive: f32,
    pub glue: f32,
    pub duck: f32,
}

impl Snap {
    pub fn read(p: &NeonParams) -> Self {
        let voices = p.voices().map(|v| VoiceSnap {
            level: v.level.value(),
            pan: v.pan.value(),
            shape: Shape {
                tune: v.tune.value(),
                decay: v.decay.value(),
                tone: v.tone.value(),
                a: v.a.value(),
                b: v.b.value(),
            },
            send: v.send.value(),
            mute: v.mute.value(),
            sample_mix: v.sample_mix.value(),
        });
        Self {
            voices,
            mode: p.mode.value(),
            slot: (p.slot.value() - 1).clamp(0, SLOTS as i32 - 1) as usize,
            kick_out: p.kick_out.value(),
            swing: p.swing.value(),
            human: p.human.value(),
            fill: p.fill.value(),
            bpm: p.bpm.value(),
            rev_size: p.rev_size.value(),
            rev_gate: p.rev_gate.value(),
            rev_tone: p.rev_tone.value(),
            rev_level: p.rev_level.value(),
            punch: p.punch.value(),
            drive: p.drive.value(),
            glue: p.glue.value(),
            duck: p.duck.value(),
        }
    }

    /// Set one parameter by its id and plain value, the way a preset stores it. Returns false for
    /// an unknown id. The plugin itself never needs this (the host sets parameters); it lets
    /// tests and tools build the engine state of a preset without a host.
    pub fn set(&mut self, id: &str, plain: f32) -> bool {
        let on = plain > 0.5;
        let index = plain.round().max(0.0) as usize;
        match id {
            "gain" => {}
            "mode" => self.mode = SeqMode::from_index(index),
            "slot" => self.slot = (index.max(1) - 1).min(SLOTS - 1),
            "kick_out" => self.kick_out = KickOut::from_index(index),
            "swing" => self.swing = plain,
            "human" => self.human = plain,
            "fill" => self.fill = Fill::from_index(index),
            "bpm" => self.bpm = plain,
            "rev_size" => self.rev_size = plain,
            "rev_gate" => self.rev_gate = params::Gate::from_index(index),
            "rev_tone" => self.rev_tone = plain,
            "rev_lvl" => self.rev_level = plain,
            "bus_punch" => self.punch = plain,
            "bus_drive" => self.drive = plain,
            "bus_glue" => self.glue = plain,
            "bus_duck" => self.duck = plain,
            _ => {
                let Some((prefix, rest)) = id.split_once('_') else { return false };
                let Some(voice) = VOICE_INFO.iter().position(|i| i.prefix == prefix) else { return false };
                let v = &mut self.voices[voice];
                match rest {
                    "lvl" => v.level = plain,
                    "pan" => v.pan = plain,
                    "tune" => v.shape.tune = plain,
                    "dec" => v.shape.decay = plain,
                    "tone" => v.shape.tone = plain,
                    "a" => v.shape.a = plain,
                    "b" => v.shape.b = plain,
                    "send" => v.send = plain,
                    "mute" => v.mute = on,
                    "smp" => v.sample_mix = plain,
                    _ => return false,
                }
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------------------------

/// Where the song is, for one stretch of samples.
#[derive(Clone, Copy, Debug)]
pub struct Transport {
    /// Beats per minute.
    pub tempo: f32,
    /// Position in quarter notes at the first sample, when the host is playing.
    pub host_beat: Option<f64>,
    /// The PLAY button: run the selected slot on the built-in clock.
    pub internal: bool,
}

#[derive(Clone, Copy, Default)]
struct Event {
    at: u64,
    voice: u8,
    vel: f32,
}

const MAX_EVENTS: usize = 128;
/// How many pattern keys can be held at once (the most recent one plays).
const MAX_HELD: usize = 8;

pub struct Engine {
    sr: f32,
    drums: [Drum; VOICES],
    players: [Player; VOICES],
    grids: [Grid; SLOTS],
    rng: Rng,
    events: [Event; MAX_EVENTS],
    n_events: usize,
    /// Samples rendered so far; the time base of `events`.
    clock: u64,
    /// Last sixteenth-note index that was played, while the sequencer runs.
    last_tick: Option<i64>,
    running: bool,
    internal_beat: f64,
    tempo: f32,
    reverb: Reverb,
    gate: Gate,
    bus: Bus,
    duck_env: f32,
    duck_target: f32,
    k_duck_down: f32,
    k_duck_up: f32,
    /// Fill held from a MIDI key, and the key holding it.
    key_fill: Option<(u8, Fill)>,
    /// Pattern keys currently held, oldest first.
    held: [u8; MAX_HELD],
    n_held: usize,
    /// For the editor: sixteenth index inside two bars (0..32) or -1 when stopped, the slot that
    /// is playing (or -1), and a level per drum that jumps on every hit and fades.
    pub step_view: i32,
    pub slot_view: i32,
    pub hit_view: [f32; VOICES],
    /// Number of hits played per drum since the engine was created.
    pub hit_count: [u32; VOICES],
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sr: sample_rate,
            drums: std::array::from_fn(|i| Drum::new(KINDS[i], sample_rate, 0x9E37_79B9 ^ (i as u32 + 1) * 7919)),
            players: std::array::from_fn(|_| Player::default()),
            grids: [Grid::EMPTY; SLOTS],
            rng: Rng::new(0x51ED_270B),
            events: [Event::default(); MAX_EVENTS],
            n_events: 0,
            clock: 0,
            last_tick: None,
            running: false,
            internal_beat: 0.0,
            tempo: 174.0,
            reverb: Reverb::new(sample_rate),
            gate: Gate::new(sample_rate),
            bus: Bus::new(sample_rate),
            duck_env: 0.0,
            duck_target: 0.0,
            k_duck_down: 1.0 - (-1.0 / (0.002 * sample_rate)).exp(),
            k_duck_up: 1.0 - (-1.0 / (0.09 * sample_rate)).exp(),
            key_fill: None,
            held: [0; MAX_HELD],
            n_held: 0,
            step_view: -1,
            slot_view: -1,
            hit_view: [0.0; VOICES],
            hit_count: [0; VOICES],
        }
    }

    pub fn reset(&mut self) {
        self.drums.iter_mut().for_each(Drum::reset);
        self.players.iter_mut().for_each(Player::reset);
        self.n_events = 0;
        self.last_tick = None;
        self.running = false;
        self.reverb.reset();
        self.gate.reset();
        self.bus.reset();
        self.duck_env = 0.0;
        self.duck_target = 0.0;
        self.key_fill = None;
        self.n_held = 0;
        self.step_view = -1;
        self.slot_view = -1;
    }

    pub fn set_grids(&mut self, grids: [Grid; SLOTS]) {
        self.grids = grids;
    }

    pub fn grids_mut(&mut self) -> &mut [Grid; SLOTS] {
        &mut self.grids
    }

    pub fn set_sample(&mut self, voice: usize, sample: Option<Arc<Sample>>) {
        self.players[voice].set_sample(sample);
    }

    /// Is anything still sounding (hits or scheduled strokes)?
    pub fn is_busy(&self) -> bool {
        self.running
            || self.n_events > 0
            || self.drums.iter().any(Drum::is_on)
            || self.players.iter().any(Player::is_on)
    }

    // ----------------------------------------------------------------------------------- MIDI

    pub fn note_on(&mut self, note: u8, vel: f32, snap: &Snap) {
        if let Some(fill) = seq::fill_for_note(note) {
            self.key_fill = Some((note, fill));
        } else if seq::slot_for_note(note).is_some() {
            self.release_key(note);
            if self.n_held == MAX_HELD {
                self.held.copy_within(1.., 0);
                self.n_held -= 1;
            }
            self.held[self.n_held] = note;
            self.n_held += 1;
        } else if let Some(voice) = seq::voice_for_note(note) {
            self.hit(voice, vel, snap);
        }
    }

    pub fn note_off(&mut self, note: u8) {
        if matches!(self.key_fill, Some((held, _)) if held == note) {
            self.key_fill = None;
        }
        self.release_key(note);
    }

    fn release_key(&mut self, note: u8) {
        if let Some(i) = self.held[..self.n_held].iter().position(|n| *n == note) {
            self.held.copy_within(i + 1..self.n_held, i);
            self.n_held -= 1;
        }
    }

    /// Slot of the most recently pressed pattern key that is still held.
    fn key_slot(&self) -> Option<usize> {
        self.n_held.checked_sub(1).and_then(|i| seq::slot_for_note(self.held[i]))
    }

    // ------------------------------------------------------------------------------- playing

    fn hit(&mut self, voice: usize, vel: f32, snap: &Snap) {
        let v = &snap.voices[voice];
        if v.mute {
            return;
        }
        let var = snap.human * self.rng.next_bipolar();
        self.drums[voice].trigger(vel, v.shape, var);
        self.players[voice].trigger(vel, v.shape.tune + 0.2 * var, v.shape.decay, self.sr);
        if voice == HAT_C {
            self.drums[HAT_O].choke();
            self.players[HAT_O].choke(self.sr);
        }
        if voice == KICK {
            self.duck_target = vel;
        }
        if v.send > 0.02 {
            if let Some(beats) = snap.rev_gate.beats() {
                let stretch = if self.slot_view >= 0 && self.grids[self.slot_view as usize].half { 2.0 } else { 1.0 };
                let samples = beats * stretch * 60.0 / self.tempo.max(20.0) * self.sr;
                self.gate.open(samples as u32);
            }
        }
        self.hit_view[voice] = self.hit_view[voice].max(vel);
        self.hit_count[voice] = self.hit_count[voice].wrapping_add(1);
    }

    fn schedule(&mut self, delay: f32, voice: usize, vel: f32) {
        if self.n_events < MAX_EVENTS {
            self.events[self.n_events] = Event { at: self.clock + delay.max(0.0) as u64, voice: voice as u8, vel };
            self.n_events += 1;
        }
    }

    /// Play sixteenth-note `tick` (counted from the start of the song): either the fill, or the
    /// steps of `slot`. A fill replaces the pattern completely.
    fn tick(&mut self, tick: i64, slot: Option<usize>, fill: Fill, snap: &Snap) {
        let sixteenth = 60.0 / self.tempo.max(20.0) / 4.0 * self.sr;
        let bar_step = tick.rem_euclid(STEPS as i64) as usize;

        if fill != Fill::Off {
            for voice in 0..VOICES {
                let Some(hit) = fill_hit(fill, voice, bar_step) else { continue };
                let strokes = hit.ratchet.max(1) as usize;
                for k in 0..strokes {
                    // Each extra stroke is a little louder, so rolls push forward.
                    let vel = hit.vel * (0.8 + 0.2 * (k + 1) as f32 / strokes as f32);
                    self.schedule(sixteenth * k as f32 / strokes as f32, voice, vel);
                }
            }
            return;
        }
        let Some(slot) = slot else { return };
        let grid = self.grids[slot];

        for voice in 0..VOICES {
            // In half-time a drum only advances on every second sixteenth, so its sixteen steps
            // stretch over two bars: the two-step becomes a slow backbeat.
            let slow = grid.half && !(grid.keep_hats && (voice == HAT_C || voice == HAT_O));
            let (step, length) = if slow {
                if tick.rem_euclid(2) != 0 {
                    continue;
                }
                ((tick.div_euclid(2)).rem_euclid(STEPS as i64) as usize, sixteenth * 2.0)
            } else {
                (bar_step, sixteenth)
            };

            let (vel, prob) = (grid.vel[voice][step], grid.prob[voice][step]);
            if vel <= 0.005 || (prob < 0.999 && self.rng.next_f32() >= prob) {
                continue;
            }
            let mut delay = if step % 2 == 1 { snap.swing * length / 3.0 } else { 0.0 };
            let mut vel = vel;
            if snap.human > 0.0 {
                vel *= 1.0 - 0.3 * snap.human * self.rng.next_f32();
                // The kick stays on the grid; everything else may land up to 6 ms late.
                if voice != KICK {
                    delay += snap.human * self.rng.next_f32() * 0.006 * self.sr;
                }
            }
            self.schedule(delay, voice, vel);
        }
    }

    /// Render one stretch of samples. `l` and `r` are overwritten; so is `kick` (the second
    /// output) when the host provides it.
    pub fn render(&mut self, snap: &Snap, tr: &Transport, l: &mut [f32], r: &mut [f32], mut kick: Option<(&mut [f32], &mut [f32])>) {
        self.tempo = tr.tempo.clamp(20.0, 400.0);
        self.reverb.set(snap.rev_size, snap.rev_tone);
        let beat_inc = self.tempo as f64 / 60.0 / self.sr as f64;

        // What plays: a held pattern key beats everything; otherwise the selected slot, if the
        // PLAY button is on or the mode follows the host. A fill replaces whichever it is.
        let enabled = snap.mode != SeqMode::Off;
        let slot = if !enabled {
            None
        } else if let Some(slot) = self.key_slot() {
            Some(slot)
        } else if tr.internal || (snap.mode == SeqMode::Always && tr.host_beat.is_some()) {
            Some(snap.slot)
        } else {
            None
        };
        let fill = self.key_fill.map(|(_, f)| f).unwrap_or(snap.fill);
        let run = enabled && (slot.is_some() || fill != Fill::Off);
        if run && !self.running && tr.host_beat.is_none() {
            self.internal_beat = 0.0;
        }
        if !run {
            self.last_tick = None;
            self.n_events = 0;
            self.step_view = -1;
        }
        self.running = run;
        self.slot_view = if run && fill == Fill::Off { slot.map(|s| s as i32).unwrap_or(-1) } else { -1 };

        // Per-drum gains for this stretch (constant-power pan, squared level).
        let mut gains = [(0.0f32, 0.0f32, 0.0f32); VOICES];
        for (g, v) in gains.iter_mut().zip(snap.voices.iter()) {
            let level = if v.mute { 0.0 } else { v.level * v.level * 0.9 };
            let angle = (v.pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
            *g = (level * angle.cos() * std::f32::consts::SQRT_2, level * angle.sin() * std::f32::consts::SQRT_2, level * v.send);
        }
        let gated = snap.rev_gate.beats().is_some();

        for i in 0..l.len() {
            if run {
                let beat = match tr.host_beat {
                    Some(b) => b + i as f64 * beat_inc,
                    None => {
                        let b = self.internal_beat;
                        self.internal_beat += beat_inc;
                        b
                    }
                };
                let position = beat * 4.0;
                let tick = position.floor() as i64;
                if self.last_tick != Some(tick) {
                    // A key pressed (or playback started) well into a step waits for the next
                    // step instead of playing this one late.
                    let starting = self.last_tick.is_none();
                    if !(starting && position - position.floor() > 0.25) {
                        self.tick(tick, slot, fill, snap);
                    }
                    self.last_tick = Some(tick);
                    self.step_view = tick.rem_euclid(2 * STEPS as i64) as i32;
                }
            }

            // Fire every stroke that is due.
            let mut e = 0;
            while e < self.n_events {
                if self.events[e].at <= self.clock {
                    let ev = self.events[e];
                    self.n_events -= 1;
                    self.events[e] = self.events[self.n_events];
                    self.hit(ev.voice as usize, ev.vel, snap);
                } else {
                    e += 1;
                }
            }
            self.clock += 1;

            // The kick pushes everything else down for a moment (sidechain pumping).
            let k = if self.duck_target > self.duck_env { self.k_duck_down } else { self.k_duck_up };
            self.duck_env += (self.duck_target - self.duck_env) * k;
            if self.duck_env > 0.95 * self.duck_target {
                self.duck_target = 0.0;
            }
            let duck = 1.0 - snap.duck * 0.85 * self.duck_env;

            let (mut dl, mut dr, mut send) = (0.0f32, 0.0f32, 0.0f32);
            let (mut kl, mut kr) = (0.0f32, 0.0f32);
            for voice in 0..VOICES {
                let (drum, player) = (&mut self.drums[voice], &mut self.players[voice]);
                if !drum.is_on() && !player.is_on() {
                    continue;
                }
                // With a sample loaded, SAMPLE blends from the synthesized drum to the sample.
                let mix = if player.has_sample() { snap.voices[voice].sample_mix } else { 0.0 };
                let synth = drum.process() * (1.0 - mix);
                let (sl, sr) = player.process();
                let (gl, gr, gs) = gains[voice];
                let (vl, vr) = ((synth + sl * mix) * gl, (synth + sr * mix) * gr);
                if voice == KICK {
                    (kl, kr) = (vl, vr);
                    if snap.kick_out != KickOut::Separate {
                        dl += vl;
                        dr += vr;
                    }
                } else {
                    dl += vl * duck;
                    dr += vr * duck;
                }
                let dry = synth + 0.5 * (sl + sr) * mix;
                send += dry * gs * if voice == KICK { 1.0 } else { duck };
            }

            let (wl, wr) = self.reverb.process(send);
            let wet = self.gate.next(gated) * snap.rev_level * duck;
            let (bl, br) = self.bus.process(dl + wl * wet, dr + wr * wet, snap.punch, snap.drive, snap.glue);
            l[i] = bl;
            r[i] = br;
            if let Some((out_l, out_r)) = kick.as_mut() {
                let on = snap.kick_out != KickOut::Mix;
                out_l[i] = if on { kl } else { 0.0 };
                out_r[i] = if on { kr } else { 0.0 };
            }
        }

        let fade = (-(l.len() as f32) / (0.12 * self.sr)).exp();
        self.hit_view.iter_mut().for_each(|h| *h *= fade);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bank::{default_slots, PATTERN_KEY};
    use crate::seq::{pattern_by_name, SNARE};

    const SR: f32 = 48_000.0;

    fn snap() -> Snap {
        let mut s = Snap::read(&NeonParams::default());
        s.human = 0.0;
        s
    }

    /// An engine whose slot 0 holds `pattern` (the other slots keep the factory bank).
    fn engine_with(pattern: &str) -> Engine {
        let mut e = Engine::new(SR);
        for (g, slot) in e.grids_mut().iter_mut().zip(default_slots()) {
            *g = slot.grid;
        }
        e.grids_mut()[0] = Grid::from_pattern(pattern_by_name(pattern).unwrap());
        e
    }

    const HOST: Transport = Transport { tempo: 174.0, host_beat: Some(0.0), internal: false };
    const STOPPED: Transport = Transport { tempo: 174.0, host_beat: None, internal: false };
    const PLAY: Transport = Transport { tempo: 174.0, host_beat: None, internal: true };

    /// Render `bars` bars at 174 BPM. With a host transport the position advances like a song.
    fn run(engine: &mut Engine, snap: &Snap, tr: Transport, bars: f32) -> (Vec<f32>, Vec<f32>) {
        let n = (bars * 4.0 * 60.0 / 174.0 * SR) as usize;
        let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
        let mut done = 0usize;
        for (cl, cr) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
            let mut t = tr;
            t.host_beat = tr.host_beat.map(|b| b + done as f64 * 174.0 / 60.0 / SR as f64);
            engine.render(snap, &t, cl, cr, None);
            done += cl.len();
        }
        assert!(l.iter().chain(r.iter()).all(|x| x.is_finite()));
        (l, r)
    }

    #[test]
    fn nothing_plays_until_a_pattern_key_is_held() {
        let s = snap();
        let mut e = engine_with("Two-Step");
        // Host playing, default "Pattern keys" mode, no key: silent.
        let (l, _) = run(&mut e, &s, HOST, 1.0);
        assert!(l.iter().all(|x| *x == 0.0));
        assert_eq!(e.step_view, -1);

        // Hold the first pattern key for two bars: two kicks and two snares per bar.
        let mut e = engine_with("Two-Step");
        e.note_on(PATTERN_KEY, 1.0, &s);
        run(&mut e, &s, HOST, 2.0);
        assert_eq!((e.hit_count[KICK], e.hit_count[SNARE]), (4, 4));
        assert_eq!(e.slot_view, 0);

        // Release: no more hits.
        e.note_off(PATTERN_KEY);
        run(&mut e, &s, HOST, 1.0);
        assert_eq!((e.hit_count[KICK], e.hit_count[SNARE]), (4, 4));
        assert_eq!(e.slot_view, -1);
    }

    #[test]
    fn each_pattern_key_plays_its_own_slot_and_the_latest_key_wins() {
        let s = snap();
        let mut e = engine_with("Two-Step");
        // Slot 5 (index 4) is "Dancefloor": three kicks per bar and claps.
        e.note_on(PATTERN_KEY + 4, 1.0, &s);
        run(&mut e, &s, HOST, 1.0);
        assert_eq!(e.hit_count[KICK], 3);
        assert!(e.hit_count[seq::CLAP] > 0);

        let mut e = engine_with("Two-Step");
        e.note_on(PATTERN_KEY + 4, 1.0, &s);
        e.note_on(PATTERN_KEY, 1.0, &s);
        assert_eq!(e.key_slot(), Some(0));
        e.note_off(PATTERN_KEY);
        assert_eq!(e.key_slot(), Some(4));
        e.note_off(PATTERN_KEY + 4);
        assert_eq!(e.key_slot(), None);
    }

    #[test]
    fn half_time_slots_halve_the_drums_and_can_keep_the_hats() {
        let s = snap();
        let count = |half: bool, keep: bool| {
            let mut e = engine_with("Two-Step");
            e.grids_mut()[0].half = half;
            e.grids_mut()[0].keep_hats = keep;
            e.note_on(PATTERN_KEY, 1.0, &s);
            run(&mut e, &s, HOST, 2.0);
            (e.hit_count[KICK], e.hit_count[SNARE], e.hit_count[HAT_C])
        };
        assert_eq!(count(false, false), (4, 4, 16));
        assert_eq!(count(true, false), (2, 2, 8));
        assert_eq!(count(true, true), (2, 2, 16));
    }

    #[test]
    fn a_fill_replaces_the_pattern_completely() {
        let s = snap();
        let mut e = engine_with("Two-Step");
        e.note_on(PATTERN_KEY, 1.0, &s);
        e.note_on(60, 1.0, &s);
        run(&mut e, &s, HOST, 1.0);
        // Snare rush: sixteen steps with the last four doubled, one kick, and no hats at all.
        assert_eq!(e.hit_count[SNARE], 20);
        assert_eq!(e.hit_count[KICK], 1);
        assert_eq!(e.hit_count[HAT_C], 0);
        // Released: the held pattern comes back.
        e.note_off(60);
        run(&mut e, &s, Transport { host_beat: Some(4.0), ..HOST }, 1.0);
        assert_eq!(e.hit_count[SNARE], 22);
        assert!(e.hit_count[HAT_C] > 0);

        // A fill key on its own plays the fill, with no pattern key held.
        let mut e = engine_with("Two-Step");
        e.note_on(62, 1.0, &s);
        run(&mut e, &s, HOST, 1.0);
        assert!(e.hit_count[seq::TOM_HI] > 0 && e.hit_count[seq::TOM_LO] > 0);
        assert_eq!(e.hit_count[SNARE], 0);
    }

    #[test]
    fn always_on_follows_the_host_and_off_only_plays_drum_notes() {
        let mut s = snap();
        s.mode = SeqMode::Always;
        s.slot = 0;
        let mut e = engine_with("Two-Step");
        run(&mut e, &s, STOPPED, 1.0);
        assert_eq!(e.hit_count[KICK], 0);
        run(&mut e, &s, HOST, 1.0);
        assert_eq!(e.hit_count[KICK], 2);

        // The PLAY button runs the selected slot without a host.
        let mut e = engine_with("Two-Step");
        run(&mut e, &snap(), PLAY, 1.0);
        assert_eq!(e.hit_count[KICK], 2);

        s.mode = SeqMode::Off;
        let mut e = engine_with("Two-Step");
        e.note_on(PATTERN_KEY, 1.0, &s);
        e.note_on(38, 1.0, &s);
        let (l, _) = run(&mut e, &s, HOST, 1.0);
        assert_eq!((e.hit_count[KICK], e.hit_count[SNARE]), (0, 1));
        assert!(l.iter().any(|x| x.abs() > 0.05));
    }

    #[test]
    fn kick_output_carries_only_the_kick() {
        let render = |mode: KickOut| {
            let mut s = snap();
            s.kick_out = mode;
            s.punch = 0.0;
            s.drive = 0.0;
            s.glue = 0.0;
            let mut e = engine_with("Two-Step");
            e.note_on(36, 1.0, &s);
            let n = 9600;
            let (mut l, mut r, mut kl, mut kr) = (vec![0.0; n], vec![0.0; n], vec![1.0; n], vec![1.0; n]);
            e.render(&s, &STOPPED, &mut l, &mut r, Some((&mut kl, &mut kr)));
            let peak = |x: &[f32]| x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            (peak(&l), peak(&kl))
        };
        let (main, aux) = render(KickOut::Mix);
        assert!(main > 0.3 && aux == 0.0, "in mix: main {main}, kick out {aux}");
        let (main, aux) = render(KickOut::Both);
        assert!(main > 0.3 && aux > 0.3, "copy: main {main}, kick out {aux}");
        let (main, aux) = render(KickOut::Separate);
        assert!(main < 1e-4 && aux > 0.3, "separate: main {main}, kick out {aux}");

        // A snare never reaches the kick output.
        let mut s = snap();
        s.kick_out = KickOut::Both;
        let mut e = engine_with("Two-Step");
        e.note_on(38, 1.0, &s);
        let (mut l, mut r, mut kl, mut kr) = (vec![0.0; 4800], vec![0.0; 4800], vec![1.0; 4800], vec![1.0; 4800]);
        e.render(&s, &STOPPED, &mut l, &mut r, Some((&mut kl, &mut kr)));
        assert!(kl.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn a_loaded_sample_replaces_the_synth_drum() {
        let mut s = snap();
        s.punch = 0.0;
        s.drive = 0.0;
        s.glue = 0.0;
        s.voices[SNARE].send = 0.0;
        s.voices[SNARE].shape.decay = 1.0;
        // A constant sample makes the output a constant: unmistakably not the synth snare.
        let sample = Arc::new(Sample { l: vec![0.5; 4800], r: vec![0.5; 4800], rate: SR });
        let mut e = engine_with("Two-Step");
        e.set_sample(SNARE, Some(sample));
        e.note_on(38, 1.0, &s);
        let (mut l, mut r) = (vec![0.0; 2400], vec![0.0; 2400]);
        e.render(&s, &STOPPED, &mut l, &mut r, None);
        let (lo, hi) = l[100..2000].iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!(lo > 0.1 && hi - lo < 0.01, "expected a flat line, got {lo}..{hi}");

        // SAMPLE at 0 % brings the synth back.
        s.voices[SNARE].sample_mix = 0.0;
        let mut e2 = engine_with("Two-Step");
        e2.set_sample(SNARE, Some(Arc::new(Sample { l: vec![0.5; 4800], r: vec![0.5; 4800], rate: SR })));
        e2.note_on(38, 1.0, &s);
        e2.render(&s, &STOPPED, &mut l, &mut r, None);
        assert!(l[100..2000].iter().any(|x| *x < -0.02), "the synth snare should swing below zero");
    }

    #[test]
    fn gate_cuts_the_reverb_tail() {
        let tail = |gate: params::Gate| {
            let mut s = snap();
            s.rev_gate = gate;
            s.rev_size = 1.0;
            s.rev_level = 1.0;
            s.voices[SNARE].send = 1.0;
            s.voices[SNARE].shape.decay = 0.0;
            let mut e = Engine::new(SR);
            e.note_on(38, 1.0, &s);
            let (mut l, mut r) = (vec![0.0; 48_000], vec![0.0; 48_000]);
            e.render(&s, &STOPPED, &mut l, &mut r, None);
            l[28_800..].iter().map(|x| x * x).sum::<f32>().sqrt()
        };
        let (open, gated) = (tail(params::Gate::Open), tail(params::Gate::Sixteenth));
        assert!(open > 0.05, "open reverb tail is too quiet: {open}");
        assert!(gated < 0.02 * open, "gated tail {gated} vs open {open}");
    }

    #[test]
    fn swing_delays_the_off_steps() {
        let first_hat_after = |swing: f32| {
            let mut s = snap();
            s.swing = swing;
            for (i, v) in s.voices.iter_mut().enumerate() {
                v.mute = i != HAT_C;
            }
            let mut e = engine_with("Roller");
            // Keep only step 4 (index 3), an off step.
            let keep = e.grids_mut()[0].vel[HAT_C][3];
            e.grids_mut()[0].vel[HAT_C] = [0.0; STEPS];
            e.grids_mut()[0].vel[HAT_C][3] = keep;
            e.note_on(PATTERN_KEY, 1.0, &s);
            let (l, _) = run(&mut e, &s, STOPPED, 1.0);
            l.iter().position(|x| x.abs() > 0.01).unwrap()
        };
        let sixteenth = 60.0 / 174.0 / 4.0 * SR;
        let (straight, swung) = (first_hat_after(0.0), first_hat_after(1.0));
        assert!((straight as f32 - 3.0 * sixteenth).abs() < 0.002 * SR, "straight hat at {straight}");
        assert!((swung as f32 - straight as f32 - sixteenth / 3.0).abs() < 0.002 * SR, "swung hat at {swung}");
    }

    #[test]
    fn snapshot_setter_knows_every_parameter() {
        use nih_plug::prelude::Params;
        let params = NeonParams::default();
        let mut s = Snap::read(&params);
        for (id, _, _) in params.param_map() {
            assert!(s.set(&id, 0.0), "Snap::set does not handle '{id}'");
        }
        assert!(!s.set("nope", 1.0) && !s.set("k_v_1", 1.0));
        assert!(s.set("s_smp", 0.7) && s.set("rev_gate", 2.0) && s.set("slot", 5.0) && s.set("mode", 1.0));
        assert_eq!(s.voices[SNARE].sample_mix, 0.7);
        assert_eq!(s.rev_gate, params::Gate::Sixteenth);
        assert_eq!((s.slot, s.mode), (4, SeqMode::Always));
    }
}
