//! Wub Machine - a monophonic wobble-bass synthesizer (VST3 + CLAP) written with nih-plug.

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

pub mod dist;
pub mod editor;
pub mod filter;
pub mod lfo;
pub mod osc;
pub mod ott;
pub mod params;
pub mod presets;
pub mod voice;

use params::WubParams;
use plugin_common::dsp::soft_clip;
use plugin_common::midi::{decode, KeyState, MidiQueue, Msg, EXTERNAL_MIDI};
use voice::{Engine, Snap, Transport, LATENCY, MAXB};

/// State shared between the audio thread and the editor.
pub struct Shared {
    pub meter: AtomicF32,
    pub lfo1: AtomicF32,
    pub lfo2: AtomicF32,
    /// Current phase (0..1) of each LFO, for the moving playhead in the editor.
    pub phase1: AtomicF32,
    pub phase2: AtomicF32,
    /// Current pattern step, or -1 when the pattern is off.
    pub step: AtomicI32,
    /// Notes currently held, for the on-screen keyboard.
    pub keys: KeyState,
    /// Notes played on the on-screen / computer keyboard.
    pub ui_midi: MidiQueue,
    /// User preset loading / saving.
    pub io: plugin_common::preset_io::PresetIo,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            meter: AtomicF32::new(0.0),
            lfo1: AtomicF32::new(0.0),
            lfo2: AtomicF32::new(0.0),
            phase1: AtomicF32::new(0.0),
            phase2: AtomicF32::new(0.0),
            step: AtomicI32::new(-1),
            keys: KeyState::default(),
            ui_midi: MidiQueue::new(),
            io: Default::default(),
        }
    }
}

pub struct WubMachine {
    params: Arc<WubParams>,
    engine: Engine,
    snap: Snap,
    shared: Arc<Shared>,
    sample_rate: f32,
}

impl Default for WubMachine {
    fn default() -> Self {
        let params = Arc::new(WubParams::default());
        Self {
            snap: Snap::read(&params),
            engine: Engine::new(44_100.0),
            params,
            shared: Arc::new(Shared::default()),
            sample_rate: 44_100.0,
        }
    }
}

impl WubMachine {
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
            Msg::Bend(value) => self.engine.pitch_bend(value),
            Msg::Cc(cc, value) => self.engine.control_change(cc, value),
            Msg::Pressure(_) => {}
        }
    }

    fn handle_event(&mut self, event: NoteEvent<()>) {
        let msg = match event {
            NoteEvent::NoteOn { note, velocity, .. } => Msg::NoteOn(note, velocity),
            NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => Msg::NoteOff(note),
            NoteEvent::MidiPitchBend { value, .. } => Msg::Bend(value),
            NoteEvent::MidiCC { cc, value, .. } => Msg::Cc(cc, value),
            _ => return,
        };
        self.handle_msg(msg);
    }
}

impl Plugin for WubMachine {
    const NAME: &'static str = "Wub Machine";
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
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.engine = Engine::new(self.sample_rate);
        self.snap = Snap::read(&self.params);
        context.set_latency_samples(LATENCY);
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.shared.keys.clear();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Notes from the on-screen keyboard and, in the standalone app, from the MIDI ports.
        self.snap = Snap::read(&self.params);
        // Inside a DAW neither source exists: notes come only from the host, like any plugin.
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
        let tempo = transport.tempo.unwrap_or(120.0) as f32;
        let playing = transport.playing;
        let beat0 = transport.pos_beats();
        let bps = tempo as f64 / 60.0;

        let mut next_event = context.next_event();
        let mut start = 0usize;
        let mut peak = 0.0f32;

        let channels = buffer.as_slice();
        let (first, rest) = channels.split_at_mut(1);
        let (out_l, out_r) = (&mut *first[0], &mut *rest[0]);

        while start < n {
            let mut end = (start + MAXB).min(n);
            self.snap = Snap::read(&self.params);

            while let Some(event) = next_event {
                let t = event.timing() as usize;
                if t <= start {
                    self.handle_event(event);
                    next_event = context.next_event();
                } else {
                    if t < end {
                        end = t;
                    }
                    break;
                }
            }

            let tr = Transport {
                tempo,
                playing,
                beat: beat0.map(|b| b + start as f64 / self.sample_rate as f64 * bps),
            };
            let l = &mut out_l[start..end];
            let r = &mut out_r[start..end];
            self.engine.render(&self.snap, &tr, l, r);

            for i in 0..l.len() {
                let g = self.params.gain.smoothed.next();
                l[i] = soft_clip(l[i] * g);
                r[i] = soft_clip(r[i] * g);
                peak = peak.max(l[i].abs()).max(r[i].abs());
            }
            start = end;
        }

        if self.params.editor_state.is_open() {
            let old = self.shared.meter.load(Ordering::Relaxed);
            let decay = (-(n as f32) / (0.25 * self.sample_rate)).exp();
            let new = if peak > old { peak } else { old * decay + peak * (1.0 - decay) };
            self.shared.meter.store(new, Ordering::Relaxed);
            self.shared.lfo1.store(self.engine.lfo_view[0], Ordering::Relaxed);
            self.shared.lfo2.store(self.engine.lfo_view[1], Ordering::Relaxed);
            self.shared.phase1.store(self.engine.phase_view[0], Ordering::Relaxed);
            self.shared.phase2.store(self.engine.phase_view[1], Ordering::Relaxed);
            self.shared.step.store(self.engine.step_view, Ordering::Relaxed);
        }

        // Keep ringing (release tails) until the voice is idle.
        if self.engine.is_sounding() {
            ProcessStatus::KeepAlive
        } else {
            ProcessStatus::Normal
        }
    }
}

impl ClapPlugin for WubMachine {
    const CLAP_ID: &'static str = "audio.szerum.wubmachine";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Wobble bass synthesizer for dubstep and drum and bass");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for WubMachine {
    const VST3_CLASS_ID: [u8; 16] = *b"WubMachineSyn001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(WubMachine);
nih_export_vst3!(WubMachine);
