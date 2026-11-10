//! egui editor for Remedy.

use crate::env::lfo_preview;
use crate::filter::response;
use crate::params::*;
use crate::presets::{self, PRESETS};
use crate::wavetable::bank;
use crate::voice::VizFrame;
use crate::Shared;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Shape, Ui};
use plugin_common::midi::MIDI_STATUS;
use plugin_common::preset_io;
use plugin_common::ui::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// Oscillator A, filter and everything "primary".
const CYAN: Color32 = Color32::from_rgb(64, 214, 222);
/// Oscillator B, modulation.
const VIOLET: Color32 = Color32::from_rgb(160, 140, 255);
/// Sub, noise and effects.
const AMBER: Color32 = Color32::from_rgb(255, 184, 92);

#[derive(Default)]
struct UiState {
    tab: usize,
}

pub fn create(params: Arc<SzParams>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        UiState::default(),
        |ctx, _| apply_style(ctx, CYAN),
        move |ctx, setter, state| {
            ctx.request_repaint_after(Duration::from_millis(33));
            let p = &*params;
            egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ctx, |ui| {
                let o = ui.max_rect().min.to_vec2();
                let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y) + o, vec2(w, h));

                // What the newest voice is doing right now; drives every moving display.
                let viz = shared.viz.lock().map(|v| *v).unwrap_or_default();
                header(ui, setter, p, &shared, r(0.0, 0.0, 1160.0, 60.0));
                osc_panel(ui, setter, &p.a, "OSC A", CYAN, viz.active.then_some(viz.pos[0]), r(12.0, 64.0, 454.0, 232.0));
                osc_panel(ui, setter, &p.b, "OSC B", VIOLET, viz.active.then_some(viz.pos[1]), r(474.0, 64.0, 454.0, 232.0));
                sub_panel(ui, setter, p, r(936.0, 64.0, 212.0, 112.0));
                noise_panel(ui, setter, p, r(936.0, 184.0, 212.0, 112.0));
                filter_panel(ui, setter, p, viz.active.then_some(viz.cutoff), r(12.0, 304.0, 390.0, 182.0));
                env_panel(ui, setter, p, &viz, r(410.0, 304.0, 738.0, 182.0));
                bottom_panel(ui, setter, p, state, &viz, r(12.0, 494.0, 1136.0, 232.0));
                if plugin_common::midi::is_standalone() {
                    keyboard(ui, r(12.0, 734.0, 1136.0, 62.0), &shared.ui_midi, &shared.keys, CYAN);
                }
            });
        },
    )
}

// ---------------------------------------------------------------------------------------------
// Displays
// ---------------------------------------------------------------------------------------------

/// The wavetable as a stack of frames receding into the screen, with the playing frame lit.
fn wave_display(ui: &Ui, rect: Rect, table: usize, pos: f32, color: Color32, on: bool, live: Option<f32>) {
    let live = live.filter(|_| on);
    let painter = display(ui, rect);
    let bank = bank();
    let t = &bank.tables[table.min(bank.tables.len() - 1)];
    let color = if on { color } else { FAINT };
    let w = rect.width() - 62.0;
    let amp = rect.height() * 0.2;
    let mut buf = [0.0f32; 128];
    let mut frame = |f: f32| -> (Vec<Pos2>, f32) {
        t.preview(f, &mut buf);
        let x0 = rect.left() + 10.0 + f * 42.0;
        let y0 = rect.bottom() - amp - 12.0 - f * (rect.height() - 2.0 * amp - 24.0);
        let pts = buf
            .iter()
            .enumerate()
            .map(|(i, v)| pos2(x0 + w * i as f32 / (buf.len() - 1) as f32, y0 - v * amp))
            .collect();
        (pts, y0)
    };
    const GHOSTS: usize = 12;
    for k in (0..GHOSTS).rev() {
        let (pts, _) = frame(k as f32 / (GHOSTS - 1) as f32);
        painter.add(Shape::line(pts, st(1.0, color.gamma_multiply(0.16))));
    }
    // While a note sounds, the lit frame follows the modulated position; the knob's own
    // position stays visible as a thin outline.
    if let Some(live) = live.filter(|l| (l - pos).abs() > 0.004) {
        let (pts, _) = frame(pos);
        painter.add(Shape::line(pts, st(1.0, color.gamma_multiply(0.45))));
        let (pts, y0) = frame(live);
        fill_under(&painter, &pts, y0 + amp, color, 0.28);
        painter.add(Shape::line(pts, st(1.8, color)));
    } else {
        let (pts, y0) = frame(pos);
        fill_under(&painter, &pts, y0 + amp, color, if live.is_some() { 0.3 } else { 0.22 });
        painter.add(Shape::line(pts, st(1.8, color)));
    }
}

/// Live output waveform. The trace starts at a rising zero crossing so steady notes stand still.
fn scope_display(ui: &Ui, rect: Rect, shared: &Shared, color: Color32) {
    let painter = display(ui, rect);
    let Ok(buf) = shared.scope.lock() else { return };
    let n = buf.len();
    if n < 8 {
        return;
    }
    let window = n / 2;
    let start = (1..n - window).find(|&i| buf[i - 1] <= 0.0 && buf[i] > 0.0).unwrap_or(0);
    let peak = buf.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    // Gentle auto-gain so quiet patches are still visible, without blowing up silence.
    let gain = 1.0 / peak.max(0.25);
    let mid = rect.center().y;
    let pts: Vec<Pos2> = (0..window)
        .map(|i| {
            pos2(
                rect.left() + 3.0 + (rect.width() - 6.0) * i as f32 / (window - 1) as f32,
                mid - buf[start + i] * gain * (rect.height() * 0.5 - 4.0),
            )
        })
        .collect();
    let lit = peak > 0.001;
    painter.add(Shape::line(pts, st(1.4, if lit { color } else { FAINT })));
}

/// Dot marking the live position on a curve.
fn playhead(painter: &egui::Painter, at: Pos2, color: Color32) {
    painter.circle_filled(at, 5.5, color.gamma_multiply(0.3));
    painter.circle_filled(at, 3.0, TEXT);
}

fn filter_display(ui: &Ui, rect: Rect, p: &SzParams, color: Color32, live_cutoff: Option<f32>) {
    let painter = display(ui, rect);
    grid(&painter, rect, 6, 4);
    let f = &p.filter;
    let color = if f.on.value() { color } else { FAINT };
    let (kind, base_cutoff, res) = (f.kind.value(), f.cutoff.value(), f.res.value());
    let curve = |cutoff: f32| -> Vec<Pos2> {
        let n = 110;
        let (top_db, bottom_db) = (24.0f32, -42.0f32);
        (0..=n)
            .map(|i| {
                let u = i as f32 / n as f32;
                let hz = 20.0 * 1000.0f32.powf(u);
                let db = (20.0 * response(kind, cutoff, res, hz).max(1e-5).log10()).clamp(bottom_db, top_db);
                let y = rect.top() + 4.0 + (rect.height() - 8.0) * (top_db - db) / (top_db - bottom_db);
                pos2(rect.left() + 3.0 + (rect.width() - 6.0) * u, y)
            })
            .collect()
    };
    // While a note sounds the curve follows the modulated cutoff (envelope, key tracking, LFOs);
    // the knob's own setting stays as a thin outline.
    let live_cutoff = live_cutoff.filter(|_| f.on.value());
    let moved = live_cutoff.filter(|l| (l / base_cutoff).ln().abs() > 0.02);
    if moved.is_some() {
        painter.add(Shape::line(curve(base_cutoff), st(1.0, color.gamma_multiply(0.4))));
    }
    let pts = curve(moved.unwrap_or(base_cutoff));
    fill_under(&painter, &pts, rect.bottom(), color, if live_cutoff.is_some() { 0.28 } else { 0.2 });
    painter.add(Shape::line(pts, st(1.8, color)));
    painter.text(
        pos2(rect.left() + 6.0, rect.bottom() - 8.0),
        Align2::LEFT_CENTER,
        "20 Hz",
        FontId::proportional(8.0),
        FAINT,
    );
    painter.text(
        pos2(rect.right() - 6.0, rect.bottom() - 8.0),
        Align2::RIGHT_CENTER,
        "20 kHz",
        FontId::proportional(8.0),
        FAINT,
    );
}

fn env_display(ui: &Ui, rect: Rect, e: &EnvParams, color: Color32, live: Option<(u8, f32)>) {
    let painter = display(ui, rect);
    let (a, d, s, r) = (e.attack.value().sqrt(), e.decay.value().sqrt(), e.sustain.value(), e.release.value().sqrt());
    let hold = 16.0;
    let total = a + d + hold + r;
    let w = rect.width() - 12.0;
    let h = rect.height() - 14.0;
    let x = |t: f32| rect.left() + 6.0 + w * t / total;
    let y = |v: f32| rect.bottom() - 6.0 - h * v;
    let mut pts = vec![pos2(x(0.0), y(0.0)), pos2(x(a), y(1.0))];
    for i in 1..=16 {
        let u = i as f32 / 16.0;
        pts.push(pos2(x(a + d * u), y(s + (1.0 - s) * (-4.6 * u).exp())));
    }
    pts.push(pos2(x(a + d + hold), y(s)));
    for i in 1..=16 {
        let u = i as f32 / 16.0;
        pts.push(pos2(x(a + d + hold + r * u), y(s * (-4.6 * u).exp())));
    }
    fill_under(&painter, &pts, rect.bottom() - 1.0, color, 0.22);
    painter.add(Shape::line(pts, st(1.6, color)));
    for t in [a, a + d, a + d + hold] {
        painter.line_segment([pos2(x(t), rect.top() + 3.0), pos2(x(t), rect.bottom() - 3.0)], st(1.0, LINE));
    }
    // Live playhead: where the newest note is on its way through the envelope.
    if let Some((stage, v)) = live {
        let v = v.clamp(0.0, 1.0);
        // Invert the exponential segments to find how far along the level is.
        let along = |from: f32, floor: f32| {
            let span = (from - floor).max(1e-4);
            (-(((v - floor) / span).clamp(0.01, 1.0)).ln() / 4.6).clamp(0.0, 1.0)
        };
        let t = match stage {
            1 => Some(a * v),
            2 => Some(a + d * along(1.0, s)),
            3 => Some(a + d + hold * 0.5),
            4 => Some(a + d + hold + r * along(s.max(v), 0.0)),
            _ => None,
        };
        if let Some(t) = t {
            playhead(&painter, pos2(x(t), y(v)), color);
        }
    }
}

fn lfo_display(ui: &Ui, rect: Rect, shape: LfoShape, color: Color32, live: Option<(f32, f32)>) {
    let painter = display(ui, rect);
    painter.line_segment(
        [pos2(rect.left() + 2.0, rect.center().y), pos2(rect.right() - 2.0, rect.center().y)],
        st(1.0, LINE),
    );
    let n = 120;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            // Two cycles, so the repeat is visible.
            let ph = (2.0 * i as f32 / n as f32).fract().min(0.9999);
            pos2(
                rect.left() + 5.0 + (rect.width() - 10.0) * i as f32 / n as f32,
                rect.center().y - lfo_preview(shape, ph) * rect.height() * 0.36,
            )
        })
        .collect();
    fill_under(&painter, &pts, rect.center().y, color, 0.2);
    painter.add(Shape::line(pts, st(1.6, color)));
    // Live playhead (phase, value) of the newest note's LFO, on the first of the two cycles.
    if let Some((phase, value)) = live {
        let x = rect.left() + 5.0 + (rect.width() - 10.0) * 0.5 * phase.clamp(0.0, 1.0);
        painter.line_segment([pos2(x, rect.top() + 3.0), pos2(x, rect.bottom() - 3.0)], st(1.0, color.gamma_multiply(0.4)));
        playhead(&painter, pos2(x, rect.center().y - value.clamp(-1.0, 1.0) * rect.height() * 0.36), color);
    }
}

// ---------------------------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------------------------

fn header(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, shared: &Shared, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    ui.painter().text(pos2(l + 18.0, t + 25.0), Align2::LEFT_CENTER, "REMEDY", FontId::proportional(25.0), TEXT);
    spaced_text(ui.painter(), pos2(l + 19.0, t + 46.0), "WAVETABLE SYNTHESIZER", 8.5, CYAN, 1.6);
    scope_display(ui, Rect::from_min_size(pos2(l + 196.0, t + 13.0), vec2(94.0, 34.0)), shared, CYAN);

    let list: Vec<(&str, &str)> = PRESETS.iter().map(|x| (x.name, x.category)).collect();
    let current = p.preset.load(Ordering::Relaxed).max(0) as usize;
    if let Some(loaded) = shared.io.take_loaded() {
        preset_io::apply_values(p, setter, &loaded.values);
        *shared.io.current.lock().unwrap() = Some(loaded.name);
    }
    let user = shared.io.current.lock().unwrap().clone();
    if let Some(i) = preset_bar(ui, Rect::from_min_size(pos2(l + 300.0, t + 15.0), vec2(300.0, 30.0)), &list, current, user.as_deref(), CYAN) {
        presets::apply(p, setter, Some(&PRESETS[i]));
        p.preset.store(i as i32, Ordering::Relaxed);
        *shared.io.current.lock().unwrap() = None;
    }
    if button(ui, Rect::from_min_size(pos2(l + 606.0, t + 17.0), vec2(46.0, 26.0)), "preset-load", "LOAD", DIM, "Load a preset from a file (.json) saved with SAVE.") {
        shared.io.import("Remedy");
    }
    if button(ui, Rect::from_min_size(pos2(l + 656.0, t + 17.0), vec2(46.0, 26.0)), "preset-save", "SAVE", DIM, "Save the current sound - every knob and switch - to a preset file you can load later or share.") {
        let name = user.clone().unwrap_or_else(|| PRESETS[current.min(PRESETS.len() - 1)].name.to_string());
        shared.io.export("Remedy", &name, p);
    }

    dropdown(
        ui,
        setter,
        &p.voice_mode,
        Rect::from_min_size(pos2(l + 712.0, t + 19.0), vec2(84.0, 22.0)),
        "Poly plays chords. Mono plays one note at a time and re-starts the envelopes on every note. \
         Legato is mono too, but overlapping notes slide without re-starting the envelopes.",
    );
    let row = Row::new(l + 832.0, t + 26.0, 48.0);
    knob(ui, setter, &p.voices, row.at(0), Kn::S, "VOICES", CYAN, "Maximum number of notes that can sound together in Poly mode. Lower it to save CPU.");
    knob(ui, setter, &p.glide, row.at(1), Kn::S, "GLIDE", CYAN, "Time the pitch takes to slide from the previous note to the new one (Mono and Legato modes).");
    knob(ui, setter, &p.bend_range, row.at(2), Kn::S, "BEND", CYAN, "How far the pitch wheel bends the note, in semitones up and down.");

    meter(ui, Rect::from_min_size(pos2(l + 972.0, t + 21.0), vec2(116.0, 10.0)), shared.meter.load(Ordering::Relaxed), CYAN);
    let message = io_status(ui, &shared.io);
    let status = message.as_deref().or(MIDI_STATUS.get().map(|s| s.as_str())).unwrap_or("OUTPUT");
    let clip = Rect::from_min_size(pos2(l + 972.0, t + 34.0), vec2(116.0, 14.0));
    ui.painter_at(clip).text(clip.left_center(), Align2::LEFT_CENTER, status, FontId::proportional(8.5), FAINT);
    knob(ui, setter, &p.gain, pos2(l + 1124.0, t + 26.0), Kn::S, "MASTER", AMBER, "Overall output volume. A soft limiter after it catches peaks; if the meter is red, turn this down.");
}

// ---------------------------------------------------------------------------------------------
// Sound sources
// ---------------------------------------------------------------------------------------------

const WARP_TIP: &str = "Warp reshapes the waveform. Sync: restarts it faster for a tearing sweep. Bend +/-: squeezes \
    it to one side. Asym / Mirror / Twist: skew or reflect it. FM: the oscillator modulates itself for metallic tones. \
    Fold / Drive / Crush: wave-folding, saturation and bit reduction.";

fn osc_panel(ui: &mut Ui, setter: &ParamSetter, o: &OscParams, title: &str, color: Color32, live_pos: Option<f32>, rect: Rect) {
    panel(ui, rect, title, color);
    let (l, t) = (rect.left(), rect.top());
    let on = o.on.value();

    dropdown(
        ui,
        setter,
        &o.table,
        Rect::from_min_size(pos2(l + 76.0, t + 4.0), vec2(150.0, 19.0)),
        "The wavetable: a set of 64 related waveforms. WT POS moves through them.",
    );
    toggle(
        ui,
        setter,
        &o.to_filter,
        Rect::from_min_size(pos2(rect.right() - 104.0, t + 5.0), vec2(64.0, 17.0)),
        "FILTER",
        color,
        "On: this oscillator goes through the filter. Off: it bypasses the filter and goes straight to the output.",
    );
    power(ui, setter, &o.on, pos2(rect.right() - 20.0, t + 13.5), color, "Switch this oscillator on or off.");

    wave_display(ui, Rect::from_min_size(pos2(l + 10.0, t + 30.0), vec2(186.0, 112.0)), o.table.value() as usize, o.pos.value(), color, on, live_pos);

    let a = Row::new(l + 236.0, t + 56.0, 58.0);
    knob(ui, setter, &o.pos, a.at(0), Kn::L, "WT POS", color, "Position in the wavetable. Sweeping it morphs the tone - this is the main thing to modulate with an LFO or envelope.");
    knob(ui, setter, &o.semi, pos2(l + 302.0, t + 56.0), Kn::M, "SEMI", color, "Coarse tuning in semitones (12 = one octave up).");
    knob(ui, setter, &o.fine, pos2(l + 356.0, t + 56.0), Kn::M, "FINE", color, "Fine tuning in cents (100 cents = one semitone). A few cents against the other oscillator adds movement.");
    knob(ui, setter, &o.level, pos2(l + 410.0, t + 56.0), Kn::M, "LEVEL", color, "Volume of this oscillator.");

    let b = Row::new(l + 236.0, t + 116.0, 58.0);
    knob(ui, setter, &o.unison, b.at(0), Kn::M, "UNISON", color, "Number of stacked copies of the oscillator (1-16). More copies sound wider and thicker, and use more CPU.");
    knob(ui, setter, &o.detune, b.at(1), Kn::M, "DETUNE", color, "How far the unison copies are tuned apart. Small = gentle chorus, large = supersaw.");
    knob(ui, setter, &o.blend, b.at(2), Kn::M, "BLEND", color, "Volume of the detuned copies against the centre one. Low keeps the pitch focused, high is a full wall of sound.");
    knob(ui, setter, &o.spread, b.at(3), Kn::M, "STEREO", color, "How far the unison copies are spread between left and right.");

    let y = t + 186.0;
    caption(ui, pos2(l + 12.0, t + 158.0), "WARP 1");
    dropdown(ui, setter, &o.w1_mode, Rect::from_min_size(pos2(l + 12.0, t + 168.0), vec2(104.0, 20.0)), WARP_TIP);
    knob(ui, setter, &o.w1_amt, pos2(l + 140.0, y - 4.0), Kn::S, "AMOUNT", color, "Strength of warp 1.");
    caption(ui, pos2(l + 174.0, t + 158.0), "WARP 2");
    dropdown(ui, setter, &o.w2_mode, Rect::from_min_size(pos2(l + 174.0, t + 168.0), vec2(104.0, 20.0)), WARP_TIP);
    knob(ui, setter, &o.w2_amt, pos2(l + 302.0, y - 4.0), Kn::S, "AMOUNT", color, "Strength of warp 2 (applied after warp 1).");

    let c = Row::new(l + 346.0, y - 4.0, 42.0);
    knob(ui, setter, &o.pan, c.at(0), Kn::S, "PAN", color, "Left / right position of this oscillator.");
    knob(ui, setter, &o.phase, c.at(1), Kn::S, "PHASE", color, "Where in its cycle the waveform starts on each note.");
    knob(ui, setter, &o.rand, c.at(2), Kn::S, "RAND", color, "Randomises the start phase per note. 100% = smooth and natural for pads; 0% = identical, punchy attacks for basses.");
}

fn sub_panel(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, rect: Rect) {
    panel(ui, rect, "SUB", AMBER);
    let (l, t) = (rect.left(), rect.top());
    power(ui, setter, &p.sub.on, pos2(rect.right() - 20.0, t + 13.5), AMBER, "Switch the sub oscillator on or off.");
    dropdown(ui, setter, &p.sub.shape, Rect::from_min_size(pos2(l + 12.0, t + 36.0), vec2(86.0, 20.0)), "Waveform of the sub oscillator. Sine is the cleanest low end.");
    toggle(
        ui,
        setter,
        &p.sub.to_filter,
        Rect::from_min_size(pos2(l + 12.0, t + 64.0), vec2(86.0, 17.0)),
        "FILTER",
        AMBER,
        "On: the sub goes through the filter. Off (usual): it stays clean and solid whatever the filter does.",
    );
    knob(ui, setter, &p.sub.octave, pos2(l + 132.0, t + 58.0), Kn::M, "OCTAVE", AMBER, "How many octaves below the played note the sub sits.");
    knob(ui, setter, &p.sub.level, pos2(l + 182.0, t + 58.0), Kn::M, "LEVEL", AMBER, "Volume of the sub oscillator.");
}

fn noise_panel(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, rect: Rect) {
    panel(ui, rect, "NOISE", AMBER);
    let (l, t) = (rect.left(), rect.top());
    power(ui, setter, &p.noise.on, pos2(rect.right() - 20.0, t + 13.5), AMBER, "Switch the noise generator on or off.");
    toggle(
        ui,
        setter,
        &p.noise.to_filter,
        Rect::from_min_size(pos2(l + 12.0, t + 50.0), vec2(86.0, 17.0)),
        "FILTER",
        AMBER,
        "On: the noise goes through the filter (needed for the plucked-string comb sound). Off: it bypasses it.",
    );
    knob(ui, setter, &p.noise.level, pos2(l + 157.0, t + 58.0), Kn::M, "LEVEL", AMBER, "Volume of the white noise. A little adds air and breath to pads and leads.");
}

// ---------------------------------------------------------------------------------------------
// Filter and envelopes
// ---------------------------------------------------------------------------------------------

fn filter_panel(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, live_cutoff: Option<f32>, rect: Rect) {
    panel(ui, rect, "FILTER", CYAN);
    let (l, t) = (rect.left(), rect.top());
    let f = &p.filter;
    dropdown(
        ui,
        setter,
        &f.kind,
        Rect::from_min_size(pos2(l + 84.0, t + 4.0), vec2(140.0, 19.0)),
        "Filter type. Low 12 / Low 24 remove highs (24 is steeper, with a ladder character). High removes lows, Band keeps \
         a region, Notch removes one, Peak boosts one. Comb +/- adds a tuned, flanger-like resonance.",
    );
    power(ui, setter, &f.on, pos2(rect.right() - 20.0, t + 13.5), CYAN, "Switch the filter on or off.");
    filter_display(ui, Rect::from_min_size(pos2(l + 10.0, t + 30.0), vec2(170.0, 142.0)), p, CYAN, live_cutoff);

    knob(ui, setter, &f.cutoff, pos2(l + 222.0, t + 62.0), Kn::L, "CUTOFF", CYAN, "The frequency where the filter starts working. For a low-pass: lower = darker.");
    knob(ui, setter, &f.res, pos2(l + 292.0, t + 62.0), Kn::L, "RESO", CYAN, "Resonance: emphasises the frequencies right at the cutoff. High values whistle and squelch.");
    knob(ui, setter, &f.drive, pos2(l + 352.0, t + 62.0), Kn::M, "DRIVE", CYAN, "Overdrives the signal going into the filter for a thicker, grittier tone.");
    let b = Row::new(l + 222.0, t + 134.0, 65.0);
    knob(ui, setter, &f.env_amt, b.at(0), Kn::M, "ENV 2", CYAN, "How far envelope 2 opens (right of centre) or closes (left) the cutoff on every note.");
    knob(ui, setter, &f.keytrack, b.at(1), Kn::M, "KEY TRK", CYAN, "Makes the cutoff follow the keyboard, so high notes stay as bright as low ones. 100% = cutoff tracks the pitch exactly.");
    knob(ui, setter, &f.mix, b.at(2), Kn::M, "MIX", CYAN, "Blend between the unfiltered (0%) and filtered (100%) signal.");
}

fn env_panel(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, viz: &VizFrame, rect: Rect) {
    panel(ui, rect, "ENVELOPES", VIOLET);
    let (l, t) = (rect.left(), rect.top());
    let info = [
        ("ENV 1  ·  AMP", CYAN, "the volume of each note"),
        ("ENV 2  ·  FILTER", AMBER, "the filter cutoff (via the ENV 2 knob) and anything you route it to in the matrix"),
        ("ENV 3  ·  MOD", VIOLET, "whatever you route it to in the matrix"),
    ];
    for (i, (name, color, what)) in info.iter().enumerate() {
        let e = &p.env[i];
        let x0 = l + 12.0 + i as f32 * 241.0;
        ui.painter().text(pos2(x0 + 232.0, t + 14.0), Align2::RIGHT_CENTER, *name, FontId::proportional(9.0), *color);
        env_display(ui, Rect::from_min_size(pos2(x0, t + 28.0), vec2(232.0, 84.0)), e, *color, viz.active.then_some(viz.env[i]));
        let row = Row::new(x0 + 29.0, t + 138.0, 58.0);
        knob(ui, setter, &e.attack, row.at(0), Kn::S, "ATTACK", *color, &format!("Time to rise from silence to full level when a key is pressed. Shapes {what}."));
        knob(ui, setter, &e.decay, row.at(1), Kn::S, "DECAY", *color, "Time to fall from the peak down to the sustain level.");
        knob(ui, setter, &e.sustain, row.at(2), Kn::S, "SUSTAIN", *color, "Level that is held for as long as the key stays down. 0% makes a pluck.");
        knob(ui, setter, &e.release, row.at(3), Kn::S, "RELEASE", *color, "Time to fade out after the key is released.");
    }
}

// ---------------------------------------------------------------------------------------------
// Bottom section: LFOs / modulation matrix / effects
// ---------------------------------------------------------------------------------------------

fn bottom_panel(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, state: &mut UiState, viz: &VizFrame, rect: Rect) {
    ui.painter().rect_filled(rect, 8.0, PANEL);
    let (l, t) = (rect.left(), rect.top());
    let tabs = [
        ("LFOS", "Four low-frequency oscillators: slow, repeating movement for the modulation matrix."),
        ("MATRIX", "The modulation matrix: connect any source (LFO, envelope, velocity, wheel, macro) to any destination."),
        ("EFFECTS", "Distortion, chorus, delay and reverb on the whole sound."),
    ];
    let mut x = l + 12.0;
    for (i, (name, tip)) in tabs.iter().enumerate() {
        let w = 78.0;
        let r = Rect::from_min_size(pos2(x, t + 5.0), vec2(w, 20.0));
        let resp = ui.interact(r, egui::Id::new(("tab", i)), egui::Sense::click());
        if resp.clicked() {
            state.tab = i;
        }
        let active = state.tab == i;
        if active {
            ui.painter().rect_filled(r, 5.0, RAISED);
            ui.painter().rect_filled(Rect::from_min_size(pos2(r.left() + 10.0, r.bottom() - 2.0), vec2(w - 20.0, 2.0)), 1.0, VIOLET);
        }
        let tw = name.len() as f32 * 7.6;
        spaced_text(
            ui.painter(),
            pos2(r.center().x - tw * 0.5, r.center().y - 0.5),
            name,
            10.5,
            if active { TEXT } else if resp.hovered() { DIM } else { FAINT },
            1.4,
        );
        resp.on_hover_text(*tip);
        x += w + 4.0;
    }
    match state.tab {
        0 => lfo_tab(ui, setter, p, viz, rect),
        1 => matrix_tab(ui, setter, p, rect),
        _ => fx_tab(ui, setter, p, rect),
    }
}

fn lfo_tab(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, viz: &VizFrame, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    for i in 0..4 {
        let lfo = &p.lfo[i];
        let x0 = l + 12.0 + i as f32 * 281.0;
        ui.painter().text(pos2(x0 + 2.0, t + 46.0), Align2::LEFT_CENTER, format!("LFO {}", i + 1), FontId::proportional(11.0), VIOLET);
        dropdown(ui, setter, &lfo.shape, Rect::from_min_size(pos2(x0 + 48.0, t + 36.0), vec2(118.0, 20.0)), "Shape of the movement. Sample & Hold jumps to a new random value each cycle; Smooth Random glides between them.");
        toggle(ui, setter, &lfo.sync, Rect::from_min_size(pos2(x0 + 172.0, t + 37.0), vec2(44.0, 18.0)), "SYNC", VIOLET, "On: the speed follows the song tempo (pick a note length). Off: the speed is set in Hz.");
        toggle(ui, setter, &lfo.retrig, Rect::from_min_size(pos2(x0 + 220.0, t + 37.0), vec2(48.0, 18.0)), "RETRIG", VIOLET, "On: the LFO restarts from the beginning on every note. Off: it runs freely, so each note catches it somewhere else.");
        lfo_display(ui, Rect::from_min_size(pos2(x0, t + 64.0), vec2(268.0, 92.0)), lfo.shape.value(), VIOLET, viz.active.then_some((viz.lfo_phase[i], viz.lfo_value[i])));
        caption(ui, pos2(x0 + 2.0, t + 190.0), "RATE");
        if lfo.sync.value() {
            dropdown(ui, setter, &lfo.div, Rect::from_min_size(pos2(x0 + 40.0, t + 180.0), vec2(84.0, 20.0)), "Length of one LFO cycle in note values at the song tempo.");
        } else {
            knob(ui, setter, &lfo.rate, pos2(x0 + 62.0, t + 184.0), Kn::M, "HZ", VIOLET, "Speed of the LFO in cycles per second.");
        }
        ui.painter().text(
            pos2(x0 + 268.0, t + 190.0),
            Align2::RIGHT_CENTER,
            "Assign it in the MATRIX tab",
            FontId::proportional(9.0),
            FAINT,
        );
    }
}

fn matrix_tab(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    caption(ui, pos2(l + 14.0, t + 46.0), "MACROS");
    let macros = [&p.macro1, &p.macro2, &p.macro3, &p.macro4];
    for (i, m) in macros.iter().enumerate() {
        let c = pos2(l + 46.0 + (i % 2) as f32 * 70.0, t + 86.0 + (i / 2) as f32 * 66.0);
        knob(ui, setter, *m, c, Kn::M, &format!("MACRO {}", i + 1), VIOLET, "A free knob. Pick it as a source in the matrix to control several things at once, then automate it from your DAW.");
    }
    for col in 0..2 {
        let x = l + 196.0 + col as f32 * 472.0;
        caption(ui, pos2(x + 22.0, t + 46.0), "SOURCE");
        caption(ui, pos2(x + 166.0, t + 46.0), "DESTINATION");
        caption(ui, pos2(x + 310.0, t + 46.0), "AMOUNT");
        for row in 0..MOD_SLOTS / 2 {
            let i = col * (MOD_SLOTS / 2) + row;
            let s = &p.slots[i];
            let y = t + 58.0 + row as f32 * 32.0;
            ui.painter().text(pos2(x + 8.0, y + 11.0), Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(10.0), FAINT);
            dropdown(ui, setter, &s.src, Rect::from_min_size(pos2(x + 22.0, y), vec2(136.0, 22.0)), "What does the moving: an LFO, an envelope, how hard you play (velocity), which note, the mod wheel, aftertouch or a macro.");
            dropdown(ui, setter, &s.dst, Rect::from_min_size(pos2(x + 166.0, y), vec2(136.0, 22.0)), "What gets moved. Pitch: full amount = 2 octaves. Cutoff: full amount = 5 octaves. Everything else: full amount = the knob's whole range.");
            hslider(ui, setter, &s.amount, Rect::from_min_size(pos2(x + 310.0, y + 1.0), vec2(140.0, 20.0)), VIOLET, "How strongly the source moves the destination. Left of centre inverts the movement.");
        }
    }
}

fn fx_tab(ui: &mut Ui, setter: &ParamSetter, p: &SzParams, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    let cell = |i: usize| Rect::from_min_size(pos2(l + 12.0 + i as f32 * 281.0, t + 36.0), vec2(268.0, 186.0));
    let frame = |ui: &mut Ui, r: Rect, name: &str| {
        ui.painter().rect_filled(r, 6.0, Color32::from_rgb(21, 23, 28));
        spaced_text(ui.painter(), pos2(r.left() + 34.0, r.top() + 16.0), name, 10.5, TEXT, 1.4);
    };

    let r = cell(0);
    frame(ui, r, "DISTORTION");
    power(ui, setter, &p.dist.on, pos2(r.left() + 17.0, r.top() + 16.0), AMBER, "Switch the distortion on or off.");
    dropdown(ui, setter, &p.dist.kind, Rect::from_min_size(pos2(r.left() + 150.0, r.top() + 6.0), vec2(108.0, 20.0)), "Soft: warm saturation. Hard Clip: aggressive, buzzy. Wavefold: metallic, adds harmonics as you push it. Bit Crush: lo-fi digital grit.");
    let row = Row::new(r.left() + 88.0, r.top() + 96.0, 92.0);
    knob(ui, setter, &p.dist.drive, row.at(0), Kn::L, "DRIVE", AMBER, "How hard the signal is pushed into the distortion.");
    knob(ui, setter, &p.dist.mix, row.at(1), Kn::L, "MIX", AMBER, "Blend between the clean (0%) and distorted (100%) signal.");

    let r = cell(1);
    frame(ui, r, "CHORUS");
    power(ui, setter, &p.chorus.on, pos2(r.left() + 17.0, r.top() + 16.0), AMBER, "Switch the chorus on or off.");
    let row = Row::new(r.left() + 54.0, r.top() + 96.0, 80.0);
    knob(ui, setter, &p.chorus.rate, row.at(0), Kn::L, "RATE", AMBER, "Speed of the chorus movement. Slow = lush, fast = vibrato / rotary speaker.");
    knob(ui, setter, &p.chorus.depth, row.at(1), Kn::L, "DEPTH", AMBER, "How far the chorus voices drift in pitch.");
    knob(ui, setter, &p.chorus.mix, row.at(2), Kn::L, "MIX", AMBER, "Amount of chorus added to the sound.");

    let r = cell(2);
    frame(ui, r, "DELAY");
    power(ui, setter, &p.delay.on, pos2(r.left() + 17.0, r.top() + 16.0), AMBER, "Switch the delay on or off.");
    toggle(ui, setter, &p.delay.pingpong, Rect::from_min_size(pos2(r.left() + 176.0, r.top() + 7.0), vec2(82.0, 18.0)), "PING-PONG", AMBER, "On: the echoes bounce between left and right. Off: each side echoes on its own.");
    let row = Row::new(r.left() + 54.0, r.top() + 96.0, 80.0);
    knob(ui, setter, &p.delay.time, row.at(0), Kn::L, "TIME", AMBER, "Time between echoes. At 120 BPM: 250 ms = 1/8 note, 375 ms = dotted 1/8, 500 ms = 1/4.");
    knob(ui, setter, &p.delay.feedback, row.at(1), Kn::L, "FEEDBACK", AMBER, "How much of each echo is fed back: more = more repeats.");
    knob(ui, setter, &p.delay.mix, row.at(2), Kn::L, "MIX", AMBER, "Volume of the echoes.");

    let r = cell(3);
    frame(ui, r, "REVERB");
    power(ui, setter, &p.reverb.on, pos2(r.left() + 17.0, r.top() + 16.0), AMBER, "Switch the reverb on or off.");
    let row = Row::new(r.left() + 40.0, r.top() + 96.0, 63.0);
    knob(ui, setter, &p.reverb.size, row.at(0), Kn::M, "SIZE", AMBER, "Length of the reverb tail: small room to huge hall.");
    knob(ui, setter, &p.reverb.damp, row.at(1), Kn::M, "DAMP", AMBER, "How quickly the highs die away in the tail. More = darker, softer space.");
    knob(ui, setter, &p.reverb.width, row.at(2), Kn::M, "WIDTH", AMBER, "Stereo width of the reverb. 0% = mono.");
    knob(ui, setter, &p.reverb.mix, row.at(3), Kn::M, "MIX", AMBER, "Amount of reverb added to the sound.");
}
