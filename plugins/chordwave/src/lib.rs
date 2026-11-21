//! Chordwave - a chord, bassline and melody sketching tool (VST3 + CLAP + standalone) written with nih-plug.
//!
//! Pick a key and scale, drag chords from a palette that only shows what fits, let the suggestion
//! engine explain what could come next, generate a bassline and a melody, then get it into your DAW
//! as a MIDI file or live as MIDI from the plugin.

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

pub mod editor;
pub mod engine;
pub mod generate;
pub mod midifile;
pub mod params;
pub mod project;
pub mod render;
pub mod style;
pub mod suggest;
pub mod synth;
pub mod theory;

use engine::{build_playback, Engine, Ev, Playback};
use params::ChordwaveParams;
use plugin_common::dsp::soft_clip;
use plugin_common::midi::{decode, KeyState, MidiQueue, Msg, EXTERNAL_MIDI};
use synth::{Synth, PART_CHORDS, PART_MELODY};
use theory::*;

/// State shared between the audio thread and the editor.
pub struct Shared {
    pub meter: AtomicF32,
    /// Position inside the loop in beats while playing, or -1.
    pub playhead: AtomicF32,
    /// The PLAY button: runs the sketch on the built-in clock while the host is stopped.
    pub play: AtomicBool,
    /// The rendered sketch for playback, rebuilt in the background whenever something changes.
    pub playback: Mutex<Playback>,
    pub playback_version: AtomicU32,
    /// Audition notes from the editor (clicking a chord, the piano roll...). Channel = part.
    pub pads: MidiQueue,
    /// Notes from the on-screen keyboard (standalone only).
    pub ui_midi: MidiQueue,
    pub keys: KeyState,
    /// A short message for the editor ("Saved Chordwave.mid").
    pub status: Mutex<Option<String>>,
    /// A MIDI file the user picked for import, waiting for the editor.
    pub imported: Mutex<Option<midifile::MidiIn>>,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            meter: AtomicF32::new(0.0),
            playhead: AtomicF32::new(-1.0),
            play: AtomicBool::new(false),
            playback: Mutex::new(Playback::default()),
            playback_version: AtomicU32::new(0),
            pads: MidiQueue::new(),
            ui_midi: MidiQueue::new(),
            keys: KeyState::default(),
            status: Mutex::new(None),
            imported: Mutex::new(None),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Sig {
    project: u32,
    key: i32,
    scale: i32,
    genre: i32,
    chord_style: i32,
    bass_style: i32,
    spread: bool,
    octave: i32,
}

impl Sig {
    fn read(p: &ChordwaveParams) -> Self {
        Self {
            project: p.project.version(),
            key: p.key.value(),
            scale: p.scale.value(),
            genre: p.genre.value(),
            chord_style: p.chord_style.value(),
            bass_style: p.bass_style.value(),
            spread: p.spread.value(),
            octave: p.octave.value(),
        }
    }
}

/// Render the sketch and publish it for the audio thread. Runs off the audio thread.
fn rebuild(params: &ChordwaveParams, shared: &Shared) {
    let project = params.project.get();
    let built = build_playback(&project, &params.render_cfg());
    if let Ok(mut slot) = shared.playback.lock() {
        *slot = built;
    }
    shared.playback_version.fetch_add(1, Ordering::AcqRel);
}

#[derive(Clone, Copy)]
pub enum Task {
    Rebuild,
}

pub struct Chordwave {
    params: Arc<ChordwaveParams>,
    shared: Arc<Shared>,
    engine: Engine,
    synth: Synth,
    sample_rate: f32,
    requested: Option<Sig>,
    local_version: u32,
    was_internal: bool,
    /// Notes started by a key in chord-input mode, so the key's release can stop them.
    held: Vec<(u8, [u8; 9], u8)>,
}

impl Default for Chordwave {
    fn default() -> Self {
        Self {
            params: Arc::new(ChordwaveParams::default()),
            shared: Arc::new(Shared::default()),
            engine: Engine::default(),
            synth: Synth::new(44_100.0),
            sample_rate: 44_100.0,
            requested: None,
            local_version: 0,
            was_internal: false,
            held: Vec::with_capacity(128),
        }
    }
}

impl Chordwave {
    /// A key pressed in chord-input mode becomes the diatonic chord built on that note.
    fn chord_for_key(&self, note: u8) -> Option<([u8; 9], u8)> {
        let mode = self.params.chord_input.value();
        if mode == 0 {
            return None;
        }
        let (key, scale) = (self.params.key_pc(), self.params.scale_v());
        let degree = degree_of(key, scale, note % 12)?;
        let ext = match mode {
            1 => Ext::Triad,
            2 => Ext::Seventh,
            _ => Ext::Ninth,
        };
        let chord = diatonic_chord(key, scale, degree, ext);
        let mut out = [0u8; 9];
        let mut n = 0;
        for &i in chord.intervals().iter().take(9) {
            let pitch = note as i32 + i as i32;
            if pitch <= 127 {
                out[n] = pitch as u8;
                n += 1;
            }
        }
        Some((out, n as u8))
    }

    fn input_on(&mut self, note: u8, vel: u8, offset: u32) {
        if let Some((notes, n)) = self.chord_for_key(note) {
            if self.held.len() < 128 && n > 0 {
                self.held.push((note, notes, n));
                for &p in &notes[..n as usize] {
                    self.engine.push(Ev { offset, on: true, pitch: p, vel, part: PART_CHORDS });
                }
            }
        } else if self.params.chord_input.value() == 0 {
            self.engine.push(Ev { offset, on: true, pitch: note, vel, part: PART_MELODY });
        }
    }

    fn input_off(&mut self, note: u8, offset: u32) {
        if let Some(i) = self.held.iter().position(|h| h.0 == note) {
            let (_, notes, n) = self.held.swap_remove(i);
            for &p in &notes[..n as usize] {
                self.engine.push(Ev { offset, on: false, pitch: p, vel: 0, part: PART_CHORDS });
            }
        } else {
            self.engine.push(Ev { offset, on: false, pitch: note, vel: 0, part: PART_MELODY });
        }
    }

    fn handle_msg(&mut self, msg: Msg, offset: u32) {
        match msg {
            Msg::NoteOn(note, v) => {
                self.shared.keys.set(note, true);
                self.input_on(note, (v * 127.0).round().clamp(1.0, 127.0) as u8, offset);
            }
            Msg::NoteOff(note) => {
                self.shared.keys.set(note, false);
                self.input_off(note, offset);
            }
            _ => {}
        }
    }
}

impl Plugin for Chordwave {
    const NAME: &'static str = "Chordwave";
    const VENDOR: &'static str = "Hodiloves";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = Task;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let params = self.params.clone();
        let shared = self.shared.clone();
        Box::new(move |task| match task {
            Task::Rebuild => rebuild(&params, &shared),
        })
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.shared.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.synth = Synth::new(self.sample_rate);
        rebuild(&self.params, &self.shared);
        self.requested = Some(Sig::read(&self.params));
        self.local_version = 0;
        true
    }

    fn reset(&mut self) {
        self.synth.kill_all();
        self.engine.reset();
        self.held.clear();
        self.shared.keys.clear();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let p = self.params.clone();

        // Pick up a freshly rendered sketch; ask for a new render when anything that shapes it changed.
        let sig = Sig::read(&p);
        if self.requested != Some(sig) {
            self.requested = Some(sig);
            context.execute_background(Task::Rebuild);
        }
        let version = self.shared.playback_version.load(Ordering::Acquire);
        if version != self.local_version {
            if let Ok(pb) = self.shared.playback.try_lock() {
                self.engine.set_playback(&pb);
                self.local_version = version;
            }
        }

        let n = buffer.samples();
        let standalone = plugin_common::midi::is_standalone();
        let transport = context.transport();
        let tempo = if standalone { p.bpm.value() as f64 } else { transport.tempo.unwrap_or(p.bpm.value() as f64) };
        let bps = tempo / 60.0 / self.sample_rate as f64;
        let host_playing = !standalone && transport.playing;
        let internal = self.shared.play.load(Ordering::Relaxed);
        if internal && !self.was_internal {
            self.engine.internal_beat = 0.0;
        }
        self.was_internal = internal;
        let (playing, beat0, from_host) = match (host_playing, internal) {
            (true, _) => (true, transport.pos_beats().unwrap_or(0.0), true),
            (false, true) => (true, self.engine.internal_beat, false),
            _ => (false, 0.0, false),
        };
        let repeat = p.repeat.value();
        let parts = [p.chords_on.value(), p.bass_on.value(), p.melody_on.value()];
        self.engine.process_block(beat0, n, bps, playing, repeat, parts);
        if playing && !from_host {
            self.engine.internal_beat += n as f64 * bps;
        }

        // Notes from the editor (auditioning), the on-screen keyboard / MIDI ports and the host.
        while let Some(raw) = self.shared.pads.pop() {
            let part = raw[0] & 0x0F;
            match decode(raw) {
                Some(Msg::NoteOn(note, v)) => self.engine.push(Ev { offset: 0, on: true, pitch: note, vel: (v * 127.0).round().max(1.0) as u8, part }),
                Some(Msg::NoteOff(note)) => self.engine.push(Ev { offset: 0, on: false, pitch: note, vel: 0, part }),
                _ => {}
            }
        }
        while let Some(raw) = standalone.then(|| self.shared.ui_midi.pop()).flatten() {
            if let Some(msg) = decode(raw) {
                self.handle_msg(msg, 0);
            }
        }
        while let Some(raw) = standalone.then(|| EXTERNAL_MIDI.pop()).flatten() {
            if let Some(msg) = decode(raw) {
                self.handle_msg(msg, 0);
            }
        }
        while let Some(event) = context.next_event() {
            let t = (event.timing() as usize).min(n.saturating_sub(1)) as u32;
            match event {
                NoteEvent::NoteOn { note, velocity, .. } => self.handle_msg(Msg::NoteOn(note, velocity), t),
                NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => self.handle_msg(Msg::NoteOff(note), t),
                _ => {}
            }
        }
        self.engine.events.sort_unstable_by_key(|e| (e.offset, e.on as u8, e.pitch));

        // Render audio between the events and send MIDI out.
        let internal_sound = p.internal.value();
        let midi_out = p.midi_out.value();
        let sound = p.sound.value().max(0) as usize;
        let gains = [p.chords_vol.value(), p.bass_vol.value(), p.melody_vol.value()];
        let channels = buffer.as_slice();
        let (first, rest) = channels.split_at_mut(1);
        let (out_l, out_r) = (&mut *first[0], &mut *rest[0]);
        out_l.fill(0.0);
        out_r.fill(0.0);

        let mut pos = 0usize;
        for i in 0..self.engine.events.len() {
            let ev = self.engine.events[i];
            let t = (ev.offset as usize).min(n);
            if t > pos {
                self.synth.render(&mut out_l[pos..t], &mut out_r[pos..t], gains);
                pos = t;
            }
            if ev.on {
                if internal_sound {
                    self.synth.note_on(ev.pitch, ev.part, sound, ev.vel);
                }
                if midi_out {
                    context.send_event(NoteEvent::NoteOn {
                        timing: t as u32,
                        voice_id: None,
                        channel: ev.part,
                        note: ev.pitch,
                        velocity: ev.vel as f32 / 127.0,
                    });
                }
            } else {
                self.synth.note_off(ev.pitch, ev.part);
                if midi_out {
                    context.send_event(NoteEvent::NoteOff { timing: t as u32, voice_id: None, channel: ev.part, note: ev.pitch, velocity: 0.0 });
                }
            }
        }
        if pos < n {
            self.synth.render(&mut out_l[pos..], &mut out_r[pos..], gains);
        }

        let mut peak = 0.0f32;
        for i in 0..n {
            let g = p.gain.smoothed.next();
            out_l[i] = soft_clip(out_l[i] * g);
            out_r[i] = soft_clip(out_r[i] * g);
            peak = peak.max(out_l[i].abs()).max(out_r[i].abs());
        }

        if p.editor_state.is_open() {
            let old = self.shared.meter.load(Ordering::Relaxed);
            let decay = (-(n as f32) / (0.25 * self.sample_rate)).exp();
            self.shared.meter.store(if peak > old { peak } else { old * decay + peak * (1.0 - decay) }, Ordering::Relaxed);
            let head = if playing { self.engine.loop_pos(beat0 + n as f64 * bps, repeat).unwrap_or(-1.0) } else { -1.0 };
            self.shared.playhead.store(head, Ordering::Relaxed);
        }

        if self.synth.is_busy() {
            ProcessStatus::KeepAlive
        } else {
            ProcessStatus::Tail((self.sample_rate * 2.0) as u32)
        }
    }
}

impl ClapPlugin for Chordwave {
    const CLAP_ID: &'static str = "audio.hodiloves.chordwave";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Chord, bass and melody sketching tool with theory-aware suggestions and MIDI export");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::NoteEffect, ClapFeature::Utility, ClapFeature::Stereo];
}

impl Vst3Plugin for Chordwave {
    const VST3_CLASS_ID: [u8; 16] = *b"ChordwaveHodi001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Tools];
}

nih_export_clap!(Chordwave);
nih_export_vst3!(Chordwave);
