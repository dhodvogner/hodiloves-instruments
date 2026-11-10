//! Neon Break - a synthesized drum machine (VST3 + CLAP + standalone) written with nih-plug.
//!
//! Eight drums built from oscillators and noise, an 80s gated reverb, a drum bus and a sixteen-step
//! sequencer with drum and bass patterns, fills and a half-time switch.

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

pub mod bank;
pub mod drums;
pub mod editor;
pub mod engine;
pub mod fx;
pub mod params;
pub mod presets;
pub mod sample;
pub mod seq;

use engine::{Engine, Snap, Transport};
use params::{NeonParams, VOICES};
use plugin_common::dsp::soft_clip;
use plugin_common::midi::{decode, KeyState, MidiQueue, Msg, EXTERNAL_MIDI};

/// State shared between the audio thread and the editor.
pub struct Shared {
    pub meter: AtomicF32,
    /// Sixteenth-note position inside two bars (0..32), or -1 while the pattern is stopped.
    pub step: AtomicI32,
    /// Pattern slot that is playing right now, or -1.
    pub playing_slot: AtomicI32,
    /// User pattern loading / saving (single slots).
    pub pattern_io: plugin_common::preset_io::PresetIo,
    /// A level per drum that jumps on every hit and fades, for the flashing pads.
    pub hits: [AtomicF32; VOICES],
    /// The PLAY button: runs the pattern on the built-in clock while the host is stopped.
    pub play: AtomicBool,
    /// Notes currently held, for the on-screen keyboard.
    pub keys: KeyState,
    /// Notes played on the on-screen / computer keyboard (standalone only).
    pub ui_midi: MidiQueue,
    /// Hits from the audition pads in the editor (always active).
    pub pads: MidiQueue,
    /// User preset loading / saving.
    pub io: plugin_common::preset_io::PresetIo,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            meter: AtomicF32::new(0.0),
            step: AtomicI32::new(-1),
            playing_slot: AtomicI32::new(-1),
            pattern_io: Default::default(),
            hits: std::array::from_fn(|_| AtomicF32::new(0.0)),
            play: AtomicBool::new(false),
            keys: KeyState::default(),
            ui_midi: MidiQueue::new(),
            pads: MidiQueue::new(),
            io: Default::default(),
        }
    }
}

pub struct NeonBreak {
    params: Arc<NeonParams>,
    engine: Engine,
    snap: Snap,
    shared: Arc<Shared>,
    sample_rate: f32,
    /// Versions of the pattern bank and the sample slots the engine currently holds.
    bank_version: u32,
    sample_version: u32,
}

impl Default for NeonBreak {
    fn default() -> Self {
        let params = Arc::new(NeonParams::default());
        Self {
            snap: Snap::read(&params),
            engine: Engine::new(44_100.0),
            params,
            shared: Arc::new(Shared::default()),
            sample_rate: 44_100.0,
            bank_version: 0,
            sample_version: 0,
        }
    }
}

impl NeonBreak {
    /// Pick up edited patterns and newly loaded samples. Never blocks: if the editor is writing
    /// right now, the old data stays for one more block.
    fn sync_shared_data(&mut self) {
        let version = self.params.bank.version();
        if version != self.bank_version && self.params.bank.try_grids(self.engine.grids_mut()) {
            self.bank_version = version;
        }
        let version = self.params.samples.version();
        if version != self.sample_version {
            let mut complete = true;
            for voice in 0..VOICES {
                match self.params.samples.try_get(voice) {
                    Ok(sample) => self.engine.set_sample(voice, sample),
                    Err(()) => complete = false,
                }
            }
            if complete {
                self.sample_version = version;
            }
        }
    }

    fn handle_msg(&mut self, msg: Msg) {
        match msg {
            Msg::NoteOn(note, velocity) => {
                self.shared.keys.set(note, true);
                self.engine.note_on(note, velocity, &self.snap);
            }
            Msg::NoteOff(note) => {
                self.shared.keys.set(note, false);
                self.engine.note_off(note);
            }
            _ => {}
        }
    }

    fn handle_event(&mut self, event: NoteEvent<()>) {
        match event {
            NoteEvent::NoteOn { note, velocity, .. } => self.handle_msg(Msg::NoteOn(note, velocity)),
            NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => self.handle_msg(Msg::NoteOff(note)),
            _ => {}
        }
    }
}

impl Plugin for NeonBreak {
    const NAME: &'static str = "Neon Break";
    const VENDOR: &'static str = "Hodiloves";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The second stereo output carries the kick on its own (see the KICK OUT setting), so a bass
    // can be sidechained to the kick alone. Hosts without multiple outputs get the plain layout.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            aux_output_ports: &[new_nonzero_u32(2)],
            names: PortNames {
                layout: Some("Stereo + Kick"),
                main_output: Some("Mix"),
                aux_outputs: &["Kick"],
                ..PortNames::const_default()
            },
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: None,
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
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
        self.engine = Engine::new(self.sample_rate);
        self.snap = Snap::read(&self.params);
        // Samples named in a restored project are read here, off the audio thread.
        if self.params.samples.take_pending() {
            self.params.samples.load_all();
        }
        self.bank_version = 0;
        self.sample_version = 0;
        self.sync_shared_data();
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.shared.keys.clear();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.snap = Snap::read(&self.params);
        self.sync_shared_data();

        // The audition pads work everywhere. The on-screen keyboard and the MIDI ports only exist
        // in the standalone app: inside a DAW, notes come from the host like for any plugin.
        while let Some(raw) = self.shared.pads.pop() {
            if let Some(Msg::NoteOn(note, velocity)) = decode(raw) {
                self.engine.note_on(note, velocity, &self.snap);
                self.engine.note_off(note);
            }
        }
        let standalone = plugin_common::midi::is_standalone();
        while let Some(raw) = standalone.then(|| self.shared.ui_midi.pop()).flatten() {
            if let Some(msg) = decode(raw) {
                self.handle_msg(msg);
            }
        }
        while let Some(raw) = standalone.then(|| EXTERNAL_MIDI.pop()).flatten() {
            if let Some(msg) = decode(raw) {
                self.handle_msg(msg);
            }
        }

        let n = buffer.samples();
        let transport = context.transport();
        // The standalone app has no song: it uses the TEMPO knob and the PLAY button.
        let tempo = if standalone { self.snap.bpm } else { transport.tempo.unwrap_or(174.0) as f32 };
        let host_beat = if !standalone && transport.playing { transport.pos_beats() } else { None };
        let internal = self.shared.play.load(Ordering::Relaxed);
        let beats_per_sample = tempo as f64 / 60.0 / self.sample_rate as f64;

        let mut next_event = context.next_event();
        let mut start = 0usize;
        let mut peak = 0.0f32;

        let channels = buffer.as_slice();
        let (first, rest) = channels.split_at_mut(1);
        let (out_l, out_r) = (&mut *first[0], &mut *rest[0]);

        // The kick output, when the host has connected it.
        let mut kick_out = aux.outputs.first_mut().and_then(|buffer| {
            let channels = buffer.as_slice();
            if channels.len() < 2 {
                return None;
            }
            let (a, b) = channels.split_at_mut(1);
            Some((&mut *a[0], &mut *b[0]))
        });

        while start < n {
            // Handle every event due now; render up to the next one.
            let mut end = n;
            while let Some(event) = next_event {
                let t = event.timing() as usize;
                if t <= start {
                    self.handle_event(event);
                    next_event = context.next_event();
                } else {
                    end = t.min(n);
                    break;
                }
            }

            let tr = Transport { tempo, host_beat: host_beat.map(|b| b + start as f64 * beats_per_sample), internal };
            let l = &mut out_l[start..end];
            let r = &mut out_r[start..end];
            let mut kick = kick_out.as_mut().map(|(kl, kr)| (&mut kl[start..end], &mut kr[start..end]));
            self.engine.render(&self.snap, &tr, l, r, kick.as_mut().map(|(kl, kr)| (&mut **kl, &mut **kr)));

            for i in 0..l.len() {
                let g = self.params.gain.smoothed.next();
                l[i] = soft_clip(l[i] * g);
                r[i] = soft_clip(r[i] * g);
                peak = peak.max(l[i].abs()).max(r[i].abs());
                if let Some((kl, kr)) = kick.as_mut() {
                    kl[i] = soft_clip(kl[i] * g);
                    kr[i] = soft_clip(kr[i] * g);
                }
            }
            start = end;
        }

        if self.params.editor_state.is_open() {
            let old = self.shared.meter.load(Ordering::Relaxed);
            let decay = (-(n as f32) / (0.25 * self.sample_rate)).exp();
            let new = if peak > old { peak } else { old * decay + peak * (1.0 - decay) };
            self.shared.meter.store(new, Ordering::Relaxed);
            self.shared.step.store(self.engine.step_view, Ordering::Relaxed);
            self.shared.playing_slot.store(self.engine.slot_view, Ordering::Relaxed);
            for (shared, level) in self.shared.hits.iter().zip(self.engine.hit_view.iter()) {
                shared.store(*level, Ordering::Relaxed);
            }
        }

        // Keep running while anything rings; afterwards allow for the reverb tail.
        if self.engine.is_busy() {
            ProcessStatus::KeepAlive
        } else {
            ProcessStatus::Tail((self.sample_rate * 5.0) as u32)
        }
    }
}

impl ClapPlugin for NeonBreak {
    const CLAP_ID: &'static str = "audio.hodiloves.neonbreak";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Synthesized 80s drum machine with gated reverb and drum and bass patterns");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::DrumMachine, ClapFeature::Drum, ClapFeature::Stereo];
}

impl Vst3Plugin for NeonBreak {
    const VST3_CLASS_ID: [u8; 16] = *b"NeonBreakDrum001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Drum];
}

nih_export_clap!(NeonBreak);
nih_export_vst3!(NeonBreak);
