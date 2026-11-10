//! Remedy - a wavetable synthesizer plugin (VST3 + CLAP + standalone) written in Rust with nih-plug.

use nih_plug::prelude::*;
use plugin_common::dsp::DcBlock;
use plugin_common::midi::{decode, KeyState, MidiQueue, Msg, EXTERNAL_MIDI};
use std::num::NonZeroU32;
use std::sync::Arc;

pub mod editor;
pub mod env;
pub mod filter;
pub mod fx;
pub mod osc;
pub mod params;
pub mod presets;
pub mod util;
pub mod voice;
pub mod wavetable;

use fx::FxChain;
use params::SzParams;
use voice::{Engine, LATENCY, MAXB};

/// Number of points in the editor's oscilloscope (every second output sample is kept).
pub const SCOPE_LEN: usize = 512;

/// State shared between the audio thread and the editor.
pub struct Shared {
    /// What the newest voice is doing (wavetable position, cutoff, envelopes, LFOs).
    pub viz: std::sync::Mutex<voice::VizFrame>,
    /// The most recent output, oldest sample first, for the oscilloscope.
    pub scope: std::sync::Mutex<Vec<f32>>,
    pub meter: AtomicF32,
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
            viz: Default::default(),
            scope: std::sync::Mutex::new(vec![0.0; SCOPE_LEN]),
            meter: AtomicF32::new(0.0),
            keys: KeyState::default(),
            ui_midi: MidiQueue::new(),
            io: Default::default(),
        }
    }
}

pub struct Remedy {
    params: Arc<SzParams>,
    engine: Engine,
    fx: FxChain,
    dc: [DcBlock; 2],
    scope_ring: [f32; SCOPE_LEN],
    scope_pos: usize,
    scope_skip: bool,
    shared: Arc<Shared>,
    sample_rate: f32,
}

impl Default for Remedy {
    fn default() -> Self {
        let params = Arc::new(SzParams::default());
        let sr = 44_100.0;
        Self {
            engine: Engine::new(sr, &params),
            fx: FxChain::new(sr),
            dc: [DcBlock::default(); 2],
            scope_ring: [0.0; SCOPE_LEN],
            scope_pos: 0,
            scope_skip: false,
            params,
            shared: Arc::new(Shared::default()),
            sample_rate: sr,
        }
    }
}

impl Remedy {
    fn handle_msg(&mut self, msg: Msg) {
        match msg {
            Msg::NoteOn(note, velocity) => {
                self.shared.keys.set(note, true);
                self.engine.note_on(note, velocity);
            }
            Msg::NoteOff(note) => {
                self.shared.keys.set(note, false);
                self.engine.note_off(note);
            }
            Msg::Bend(value) => self.engine.pitch_bend(value),
            Msg::Cc(cc, value) => self.engine.control_change(cc, value),
            Msg::Pressure(value) => self.engine.channel_pressure(value),
        }
    }

    fn handle_event(&mut self, event: NoteEvent<()>) {
        let msg = match event {
            NoteEvent::NoteOn { note, velocity, .. } => Msg::NoteOn(note, velocity),
            NoteEvent::NoteOff { note, .. } | NoteEvent::Choke { note, .. } => Msg::NoteOff(note),
            NoteEvent::MidiPitchBend { value, .. } => Msg::Bend(value),
            NoteEvent::MidiCC { cc, value, .. } => Msg::Cc(cc, value),
            NoteEvent::MidiChannelPressure { pressure, .. } => Msg::Pressure(pressure),
            _ => return,
        };
        self.handle_msg(msg);
    }
}

impl Plugin for Remedy {
    const NAME: &'static str = "Remedy";
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
        self.engine = Engine::new(self.sample_rate, &self.params);
        self.fx = FxChain::new(self.sample_rate);
        // Build the wavetable bank here so the audio thread never pays for it.
        let _ = wavetable::bank();
        context.set_latency_samples(LATENCY);
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.fx.reset();
        self.shared.keys.clear();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Notes from the on-screen keyboard and, in the standalone app, from the MIDI ports.
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
        let tempo = context.transport().tempo;
        let mut next_event = context.next_event();
        let mut start = 0usize;
        let mut peak = 0.0f32;

        let channels = buffer.as_slice();
        let (first, rest) = channels.split_at_mut(1);
        let (out_l, out_r) = (&mut *first[0], &mut *rest[0]);

        while start < n {
            let mut end = (start + MAXB).min(n);

            // Handle every event due at or before `start`; shorten the block to the next one.
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

            let l = &mut out_l[start..end];
            let r = &mut out_r[start..end];
            self.engine.render(&self.params, tempo, l, r);
            self.fx.process(&self.params, l, r);

            for i in 0..l.len() {
                let g = self.params.gain.smoothed.next();
                l[i] = util::soft_clip(self.dc[0].process(l[i]) * g);
                r[i] = util::soft_clip(self.dc[1].process(r[i]) * g);
                peak = peak.max(l[i].abs()).max(r[i].abs());
                self.scope_skip = !self.scope_skip;
                if self.scope_skip {
                    self.scope_ring[self.scope_pos] = 0.5 * (l[i] + r[i]);
                    self.scope_pos = (self.scope_pos + 1) % SCOPE_LEN;
                }
            }
            start = end;
        }

        // Live displays and the peak meter (with a slow fall-off) for the editor.
        if self.params.editor_state.is_open() {
            // `try_lock` never blocks the audio thread; a missed frame is simply drawn next time.
            if let Ok(mut viz) = self.shared.viz.try_lock() {
                *viz = self.engine.viz;
            }
            if let Ok(mut scope) = self.shared.scope.try_lock() {
                let (newer, older) = self.scope_ring.split_at(self.scope_pos);
                scope[..older.len()].copy_from_slice(older);
                scope[older.len()..].copy_from_slice(newer);
            }
            let old = self.shared.meter.load(std::sync::atomic::Ordering::Relaxed);
            let decay = (-(n as f32) / (0.25 * self.sample_rate)).exp();
            let new = if peak > old { peak } else { old * decay + peak * (1.0 - decay) };
            self.shared.meter.store(new, std::sync::atomic::Ordering::Relaxed);
        }

        // Effect tails (delay, reverb) keep ringing after the last voice stops.
        if self.engine.active_voices() > 0 {
            ProcessStatus::KeepAlive
        } else {
            ProcessStatus::Tail((self.sample_rate * 12.0) as u32)
        }
    }
}

impl ClapPlugin for Remedy {
    const CLAP_ID: &'static str = "audio.remedy.synth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Wavetable synthesizer with unison, warp and a modulation matrix");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for Remedy {
    const VST3_CLASS_ID: [u8; 16] = *b"RemedySynth_0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(Remedy);
nih_export_vst3!(Remedy);
