//! egui editor for the Wub Machine.

use crate::lfo::shape_value;
use crate::params::*;
use crate::presets::{self, PRESETS};
use crate::Shared;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui::{self, pos2, vec2, Align2, Color32, FontId, Id, Pos2, Rect, Sense, Shape, StrokeKind, Ui};
use plugin_common::midi::MIDI_STATUS;
use plugin_common::ui::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// Oscillator 1, filter, LFO 1 and the pattern: the "wub" itself.
const LIME: Color32 = Color32::from_rgb(186, 240, 78);
/// Oscillator 2, formant and LFO 2: the "growl".
const MAGENTA: Color32 = Color32::from_rgb(255, 98, 170);
/// Distortion and output.
const AMBER: Color32 = Color32::from_rgb(255, 184, 92);

pub fn create(params: Arc<WubParams>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        (),
        |ctx, _| apply_style(ctx, LIME),
        move |ctx, setter, _| {
            // Keep the live displays moving.
            ctx.request_repaint_after(Duration::from_millis(33));
            let p = &*params;
            egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ctx, |ui| {
                let o = ui.max_rect().min.to_vec2();
                let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y) + o, vec2(w, h));

                header(ui, setter, p, &shared, r(0.0, 0.0, 1160.0, 60.0));

                osc1_panel(ui, setter, p, r(12.0, 64.0, 356.0, 126.0));
                osc2_panel(ui, setter, p, r(376.0, 64.0, 262.0, 126.0));
                sub_panel(ui, setter, p, r(646.0, 64.0, 230.0, 126.0));
                voice_panel(ui, setter, p, r(884.0, 64.0, 264.0, 126.0));

                filter_panel(ui, setter, p, r(12.0, 198.0, 300.0, 126.0));
                formant_panel(ui, setter, p, r(320.0, 198.0, 190.0, 126.0));
                dist_panel(ui, setter, &p.dist_a, "DIST A", "First distortion stage, right after the filter.", r(518.0, 198.0, 176.0, 126.0));
                dist_panel(ui, setter, &p.dist_b, "DIST B", "Second distortion stage, after the vowel filter.", r(702.0, 198.0, 176.0, 126.0));
                output_panel(ui, setter, p, r(886.0, 198.0, 262.0, 126.0));

                let (v1, ph1) = (shared.lfo1.load(Ordering::Relaxed), shared.phase1.load(Ordering::Relaxed));
                let (v2, ph2) = (shared.lfo2.load(Ordering::Relaxed), shared.phase2.load(Ordering::Relaxed));
                lfo_panel(ui, setter, &p.lfo1, "LFO 1  ·  WUB", LIME, v1, ph1, r(12.0, 332.0, 430.0, 192.0));
                lfo_panel(ui, setter, &p.lfo2, "LFO 2  ·  GROWL", MAGENTA, v2, ph2, r(450.0, 332.0, 430.0, 192.0));
                env_panel(ui, setter, p, r(888.0, 332.0, 260.0, 192.0));

                pattern_panel(ui, setter, p, shared.step.load(Ordering::Relaxed), r(12.0, 532.0, 1136.0, 224.0));
                if plugin_common::midi::is_standalone() {
                    keyboard(ui, r(12.0, 764.0, 1136.0, 62.0), &shared.ui_midi, &shared.keys, LIME);
                }
            });
        },
    )
}

// ---------------------------------------------------------------------------------------------
// Header: preset browser, reset, output
// ---------------------------------------------------------------------------------------------

fn header(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, shared: &Shared, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    ui.painter().text(pos2(l + 18.0, t + 25.0), Align2::LEFT_CENTER, "WUB MACHINE", FontId::proportional(25.0), TEXT);
    spaced_text(ui.painter(), pos2(l + 19.0, t + 46.0), "WOBBLE BASS SYNTHESIZER", 8.5, LIME, 1.6);

    let list: Vec<(&str, &str)> = PRESETS.iter().map(|x| (x.name, x.category)).collect();
    let current = p.preset.load(Ordering::Relaxed).max(0) as usize;
    if let Some(loaded) = shared.io.take_loaded() {
        plugin_common::preset_io::apply_values(p, setter, &loaded.values);
        *shared.io.current.lock().unwrap() = Some(loaded.name);
    }
    let user = shared.io.current.lock().unwrap().clone();
    if let Some(i) = preset_bar(ui, Rect::from_min_size(pos2(l + 300.0, t + 15.0), vec2(300.0, 30.0)), &list, current, user.as_deref(), LIME) {
        presets::apply(p, setter, Some(&PRESETS[i]));
        p.preset.store(i as i32, Ordering::Relaxed);
        *shared.io.current.lock().unwrap() = None;
    }
    if button(ui, Rect::from_min_size(pos2(l + 606.0, t + 17.0), vec2(46.0, 26.0)), "preset-load", "LOAD", DIM, "Load a preset from a file (.json) saved with SAVE.") {
        shared.io.import("Wub Machine");
    }
    if button(ui, Rect::from_min_size(pos2(l + 656.0, t + 17.0), vec2(46.0, 26.0)), "preset-save", "SAVE", DIM, "Save the current sound - every knob, switch and pattern step - to a preset file you can load later or share.") {
        let name = user.clone().unwrap_or_else(|| PRESETS[current.min(PRESETS.len() - 1)].name.to_string());
        shared.io.export("Wub Machine", &name, p);
    }
    if button(ui, Rect::from_min_size(pos2(l + 708.0, t + 17.0), vec2(52.0, 26.0)), "reset", "RESET", DIM, "Reset every control to its default value.") {
        presets::apply(p, setter, None);
        p.preset.store(0, Ordering::Relaxed);
        *shared.io.current.lock().unwrap() = None;
    }

    meter(ui, Rect::from_min_size(pos2(l + 940.0, t + 21.0), vec2(148.0, 10.0)), shared.meter.load(Ordering::Relaxed), LIME);
    let message = io_status(ui, &shared.io);
    let status = message.as_deref().or(MIDI_STATUS.get().map(|s| s.as_str())).unwrap_or("OUTPUT");
    let clip = Rect::from_min_size(pos2(l + 800.0, t + 34.0), vec2(288.0, 14.0));
    ui.painter_at(clip).text(clip.right_center(), Align2::RIGHT_CENTER, status, FontId::proportional(8.5), FAINT);
    knob(ui, setter, &p.gain, pos2(l + 1124.0, t + 26.0), Kn::S, "OUTPUT", AMBER, "Overall output volume. A soft limiter after it catches peaks; if the meter is red, turn this down.");
}

// ---------------------------------------------------------------------------------------------
// Sound sources
// ---------------------------------------------------------------------------------------------

fn osc1_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "OSC 1", LIME);
    let (l, t) = (rect.left(), rect.top());
    let o = &p.osc1;
    dropdown(ui, setter, &o.wave, Rect::from_min_size(pos2(l + 74.0, t + 4.0), vec2(118.0, 19.0)), "Waveform of the main oscillator. Saw is the classic wobble source; Pulse lets PWM move its width.");
    let row = Row::new(l + 38.0, t + 66.0, 56.0);
    knob(ui, setter, &o.level, row.at(0), Kn::M, "LEVEL", LIME, "Volume of oscillator 1 going into the filter.");
    knob(ui, setter, &o.pwm, row.at(1), Kn::M, "PWM", LIME, "Pulse width (only for the Pulse wave). 50% is a square; towards the ends it gets thin and nasal.");
    knob(ui, setter, &o.voices, row.at(2), Kn::M, "UNISON", LIME, "Number of stacked, detuned copies (1-7). More = wider and thicker.");
    knob(ui, setter, &o.detune, row.at(3), Kn::M, "DETUNE", LIME, "How far the unison copies are tuned apart.");
    knob(ui, setter, &o.spread, row.at(4), Kn::M, "STEREO", LIME, "Stereo spread of the unison copies. The low end always stays mono.");
    knob(ui, setter, &o.rand, row.at(5), Kn::M, "RAND", LIME, "Randomises the start phase of each copy per note. 0% = every note starts identically (tight); higher = looser, more analog.");
}

fn osc2_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "OSC 2", MAGENTA);
    let (l, t) = (rect.left(), rect.top());
    let o = &p.osc2;
    dropdown(ui, setter, &o.wave, Rect::from_min_size(pos2(l + 74.0, t + 4.0), vec2(104.0, 19.0)), "Waveform of oscillator 2. As an FM source, Sine gives the cleanest growl.");
    let row = Row::new(l + 40.0, t + 66.0, 60.0);
    knob(ui, setter, &o.level, row.at(0), Kn::M, "LEVEL", MAGENTA, "Volume of oscillator 2 in the mix. Set it to 0 to use it only as an FM source.");
    knob(ui, setter, &o.semi, row.at(1), Kn::M, "SEMI", MAGENTA, "Tuning in semitones relative to oscillator 1. For FM, 7, 12 and 19 give musical growls.");
    knob(ui, setter, &o.fine, row.at(2), Kn::M, "FINE", MAGENTA, "Fine tuning in cents. A few cents of offset makes the two oscillators beat against each other.");
    knob(ui, setter, &o.fm, row.at(3), Kn::M, "FM > 1", MAGENTA, "Frequency modulation: oscillator 2 bends the phase of oscillator 1. This is the source of metallic, growling tones.");
}

fn sub_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "SUB", AMBER);
    let (l, t) = (rect.left(), rect.top());
    let s = &p.sub;
    dropdown(ui, setter, &s.wave, Rect::from_min_size(pos2(l + 56.0, t + 4.0), vec2(84.0, 19.0)), "Waveform of the sub oscillator. Sine is the cleanest low end.");
    toggle(ui, setter, &s.to_filter, Rect::from_min_size(pos2(rect.right() - 76.0, t + 5.0), vec2(64.0, 17.0)), "FILTER", AMBER, "Off (usual): the sub stays clean and constant under the wobble. On: the sub is filtered and distorted with everything else.");
    let row = Row::new(l + 44.0, t + 66.0, 70.0);
    knob(ui, setter, &s.level, row.at(0), Kn::M, "SUB", AMBER, "Volume of the sub oscillator.");
    knob(ui, setter, &s.octave, row.at(1), Kn::M, "OCTAVE", AMBER, "One or two octaves below the played note.");
    knob(ui, setter, &p.noise, row.at(2), Kn::M, "NOISE", AMBER, "White noise mixed into the filter input: adds breath and grit to the wobble.");
}

fn voice_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "VOICE", LIME);
    let (l, t) = (rect.left(), rect.top());
    toggle(ui, setter, &p.legato, Rect::from_min_size(pos2(rect.right() - 80.0, t + 5.0), vec2(68.0, 17.0)), "LEGATO", LIME, "On: overlapping notes slide to the new pitch without restarting the envelopes or the wobble.");
    let row = Row::new(l + 40.0, t + 66.0, 61.0);
    knob(ui, setter, &p.glide, row.at(0), Kn::M, "GLIDE", LIME, "Time the pitch takes to slide from one note to the next.");
    knob(ui, setter, &p.bend_range, row.at(1), Kn::M, "BEND", LIME, "How far the pitch wheel bends the note, in semitones.");
    knob(ui, setter, &p.vel_sens, row.at(2), Kn::M, "VELOCITY", LIME, "How much playing harder makes the note louder. 0% = always full volume.");
    knob(ui, setter, &p.wheel_amt, row.at(3), Kn::M, "WHEEL", LIME, "Lets the mod wheel control the depth of LFO 1. At 100% the wobble is off with the wheel down and full with it up.");
}

// ---------------------------------------------------------------------------------------------
// Tone shaping
// ---------------------------------------------------------------------------------------------

fn filter_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "FILTER", LIME);
    let (l, t) = (rect.left(), rect.top());
    let f = &p.filter;
    dropdown(ui, setter, &f.kind, Rect::from_min_size(pos2(l + 84.0, t + 4.0), vec2(128.0, 19.0)), "Low 24: steep ladder filter, the classic wobble. Low 12: gentler and brighter. Band 12: keeps only a band, for thin, vocal sweeps.");
    knob(ui, setter, &f.cutoff, pos2(l + 44.0, t + 62.0), Kn::L, "CUTOFF", LIME, "Resting point of the filter. The LFO sweeps upwards from here, so keep it low for a deep wobble.");
    knob(ui, setter, &f.res, pos2(l + 110.0, t + 62.0), Kn::L, "RESO", LIME, "Resonance: emphasises the cutoff frequency. Higher values make the sweep more vocal and squelchy.");
    let row = Row::new(l + 168.0, t + 66.0, 50.0);
    knob(ui, setter, &f.drive, row.at(0), Kn::M, "DRIVE", LIME, "Overdrives the signal going into the filter for a thicker tone.");
    knob(ui, setter, &f.keytrack, row.at(1), Kn::M, "KEY TRK", LIME, "Makes the cutoff follow the note, so the wobble sounds the same across the keyboard.");
    knob(ui, setter, &f.env_amt, row.at(2), Kn::M, "ENV", LIME, "How far the filter envelope opens (right of centre) or closes (left) the cutoff at the start of each note.");
}

fn formant_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "FORMANT", MAGENTA);
    let (l, t) = (rect.left(), rect.top());
    let f = &p.formant;
    ui.painter().text(pos2(rect.right() - 12.0, t + 14.0), Align2::RIGHT_CENTER, "A  E  I  O  U", FontId::proportional(9.0), FAINT);
    let row = Row::new(l + 38.0, t + 66.0, 57.0);
    knob(ui, setter, &f.mix, row.at(0), Kn::M, "MIX", MAGENTA, "Amount of the vowel filter. It makes the bass 'talk'.");
    knob(ui, setter, &f.morph, row.at(1), Kn::M, "VOWEL", MAGENTA, "Which vowel: sweeps A - E - I - O - U. Modulate it with an LFO for 'yoi yoi' growls.");
    knob(ui, setter, &f.q, row.at(2), Kn::M, "FOCUS", MAGENTA, "How narrow and pronounced the vowel is.");
}

fn dist_panel(ui: &mut Ui, setter: &ParamSetter, d: &DistParams, title: &str, what: &str, rect: Rect) {
    panel(ui, rect, title, AMBER);
    let (l, t) = (rect.left(), rect.top());
    dropdown(
        ui,
        setter,
        &d.kind,
        Rect::from_min_size(pos2(l + 76.0, t + 4.0), vec2(90.0, 19.0)),
        &format!("{what} Soft: warm. Hard Clip: buzzy and loud. Wavefold: metallic. Tube: fat, asymmetric. Rectify: octave-up rasp. Bit Crush: digital grit."),
    );
    let row = Row::new(l + 52.0, t + 66.0, 72.0);
    knob(ui, setter, &d.drive, row.at(0), Kn::M, "DRIVE", AMBER, "How hard the signal is pushed into the distortion.");
    knob(ui, setter, &d.mix, row.at(1), Kn::M, "MIX", AMBER, "Blend between the clean (0%) and distorted (100%) signal.");
}

fn output_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "TONE", AMBER);
    let (l, t) = (rect.left(), rect.top());
    let row = Row::new(l + 44.0, t + 66.0, 62.0);
    knob(ui, setter, &p.tone.low_cut, row.at(0), Kn::M, "LOW CUT", AMBER, "Removes rumble below this frequency. Raise it to make room for a separate sub bass.");
    knob(ui, setter, &p.tone.high_cut, row.at(1), Kn::M, "HIGH CUT", AMBER, "Rolls off harsh highs above this frequency.");
    knob(ui, setter, &p.tone.ott, pos2(l + 196.0, t + 62.0), Kn::L, "OTT", AMBER, "Multiband compression in the 'OTT' style: evens out lows, mids and highs and pulls quiet detail forward. The modern, in-your-face bass sound.");
}

fn env_display(ui: &Ui, rect: Rect, e: &EnvParams, color: Color32, name: &str) {
    let painter = display(ui, rect);
    let (a, d, s, r) = (e.attack.value().sqrt(), e.decay.value().sqrt(), e.sustain.value(), e.release.value().sqrt());
    let hold = 12.0;
    let total = a + d + hold + r;
    let w = rect.width() - 10.0;
    let h = rect.height() - 22.0;
    let x = |t: f32| rect.left() + 5.0 + w * t / total;
    let y = |v: f32| rect.bottom() - 5.0 - h * v;
    let mut pts = vec![pos2(x(0.0), y(0.0)), pos2(x(a), y(1.0))];
    for i in 1..=12 {
        let u = i as f32 / 12.0;
        pts.push(pos2(x(a + d * u), y(s + (1.0 - s) * (-4.6 * u).exp())));
    }
    pts.push(pos2(x(a + d + hold), y(s)));
    for i in 1..=12 {
        let u = i as f32 / 12.0;
        pts.push(pos2(x(a + d + hold + r * u), y(s * (-4.6 * u).exp())));
    }
    fill_under(&painter, &pts, rect.bottom() - 1.0, color, 0.22);
    painter.add(Shape::line(pts, st(1.5, color)));
    painter.text(pos2(rect.left() + 6.0, rect.top() + 9.0), Align2::LEFT_CENTER, name, FontId::proportional(8.5), color);
}

fn env_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, rect: Rect) {
    panel(ui, rect, "ENVELOPES", LIME);
    let (l, t) = (rect.left(), rect.top());
    let rows = [
        ("AMP", &p.amp_env, LIME, "the volume of the note"),
        ("FILTER", &p.filt_env, MAGENTA, "the filter cutoff (set how much with the filter's ENV knob)"),
    ];
    for (i, (name, env, color, what)) in rows.iter().enumerate() {
        let y = t + 34.0 + i as f32 * 78.0;
        env_display(ui, Rect::from_min_size(pos2(l + 10.0, y), vec2(74.0, 62.0)), env, *color, name);
        let row = Row::new(l + 110.0, y + 22.0, 40.0);
        knob(ui, setter, &env.attack, row.at(0), Kn::S, "A", *color, &format!("Attack: time to rise to full level when a key is pressed. Shapes {what}."));
        knob(ui, setter, &env.decay, row.at(1), Kn::S, "D", *color, "Decay: time to fall from the peak to the sustain level.");
        knob(ui, setter, &env.sustain, row.at(2), Kn::S, "S", *color, "Sustain: level held while the key stays down.");
        knob(ui, setter, &env.release, row.at(3), Kn::S, "R", *color, "Release: time to fade out after the key is released.");
    }
}

// ---------------------------------------------------------------------------------------------
// LFOs
// ---------------------------------------------------------------------------------------------

fn lfo_display(ui: &Ui, rect: Rect, l: &LfoParams, color: Color32, live: f32, phase: f32) {
    let painter = display(ui, rect);
    grid(&painter, rect, 4, 2);
    let bipolar = l.bipolar.value();
    let shape = l.shape.value();
    let offset = l.phase.value();
    let plot = rect.shrink2(vec2(6.0, 8.0));
    let to_y = |u: f32| plot.bottom() - u * plot.height();
    let n = 140;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let ph = (i as f32 / n as f32 + offset).fract().min(0.9999);
            let u = if shape == LfoShape::Random {
                [0.7, 0.2, 0.9, 0.4, 0.1, 0.6][((ph * 6.0) as usize).min(5)]
            } else {
                shape_value(shape, ph)
            };
            pos2(plot.left() + plot.width() * i as f32 / n as f32, to_y(u))
        })
        .collect();
    let base = if bipolar { plot.center().y } else { plot.bottom() };
    fill_under(&painter, &pts, base, color, 0.2);
    painter.add(Shape::line(pts, st(1.8, color)));
    if bipolar {
        painter.line_segment([pos2(plot.left(), base), pos2(plot.right(), base)], st(1.0, LINE));
    }
    // Live playhead: where the LFO is right now.
    let x = plot.left() + plot.width() * phase.clamp(0.0, 1.0);
    let u = if bipolar { live * 0.5 + 0.5 } else { live };
    painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], st(1.0, color.gamma_multiply(0.35)));
    painter.circle_filled(pos2(x, to_y(u.clamp(0.0, 1.0))), 3.5, TEXT);
}

#[allow(clippy::too_many_arguments)]
fn lfo_panel(ui: &mut Ui, setter: &ParamSetter, l: &LfoParams, title: &str, color: Color32, live: f32, phase: f32, rect: Rect) {
    panel(ui, rect, title, color);
    let (x0, t) = (rect.left(), rect.top());
    dropdown(ui, setter, &l.shape, Rect::from_min_size(pos2(x0 + 150.0, t + 4.0), vec2(100.0, 19.0)), "Shape of the wobble. Sine / Triangle are smooth, Saw Down is a plucky 'wow', Square is a hard gate, Random jumps to a new value each cycle.");
    toggle(ui, setter, &l.sync, Rect::from_min_size(pos2(x0 + 256.0, t + 5.0), vec2(44.0, 17.0)), "SYNC", color, "On: the speed follows the song tempo (note lengths). Off: the speed is set in Hz.");
    toggle(ui, setter, &l.bipolar, Rect::from_min_size(pos2(x0 + 304.0, t + 5.0), vec2(58.0, 17.0)), "BIPOLAR", color, "Off: the LFO only pushes its targets up from their knob position. On: it swings both above and below.");

    lfo_display(ui, Rect::from_min_size(pos2(x0 + 10.0, t + 30.0), vec2(196.0, 86.0)), l, color, live, phase);
    caption(ui, pos2(x0 + 12.0, t + 128.0), "RATE");
    if l.sync.value() {
        dropdown(ui, setter, &l.div, Rect::from_min_size(pos2(x0 + 10.0, t + 138.0), vec2(84.0, 20.0)), "Length of one wobble in note values: 1/4 = one wub per beat, 1/8 = two, 1/8T = triplets...");
    } else {
        knob(ui, setter, &l.hz, pos2(x0 + 52.0, t + 150.0), Kn::S, "HZ", color, "Speed of the LFO in cycles per second.");
    }
    caption(ui, pos2(x0 + 108.0, t + 128.0), "RESTART");
    dropdown(ui, setter, &l.trig, Rect::from_min_size(pos2(x0 + 106.0, t + 138.0), vec2(100.0, 20.0)), "Note: the wobble restarts with every new note. Grid: it is locked to the bars of your song. Free: it just keeps running.");

    let a = Row::new(x0 + 242.0, t + 48.0, 50.0);
    knob(ui, setter, &l.cutoff, a.at(0), Kn::S, "CUTOFF", color, "How far this LFO sweeps the filter cutoff. This is the wobble itself.");
    knob(ui, setter, &l.res, a.at(1), Kn::S, "RESO", color, "LFO depth to filter resonance.");
    knob(ui, setter, &l.drive, a.at(2), Kn::S, "DRIVE", color, "LFO depth to the drive of distortion A: the sound gets dirtier as the LFO rises.");
    knob(ui, setter, &l.formant, a.at(3), Kn::S, "VOWEL", color, "LFO depth to the vowel position: makes the bass 'talk' (needs FORMANT mix above 0).");
    let b = Row::new(x0 + 242.0, t + 100.0, 50.0);
    knob(ui, setter, &l.fm, b.at(0), Kn::S, "FM", color, "LFO depth to the FM amount (oscillator 2 into 1): growls that open and close.");
    knob(ui, setter, &l.pwm, b.at(1), Kn::S, "PWM", color, "LFO depth to the pulse width of oscillator 1 (Pulse wave only).");
    knob(ui, setter, &l.pitch, b.at(2), Kn::S, "PITCH", color, "LFO depth to pitch: full = one octave. Small amounts give vibrato, large ones sirens.");
    knob(ui, setter, &l.amp, b.at(3), Kn::S, "VOLUME", color, "LFO depth to volume: a tremolo / gate. Left of centre inverts it.");
    let c = Row::new(x0 + 242.0, t + 152.0, 50.0);
    knob(ui, setter, &l.phase, c.at(0), Kn::S, "PHASE", color, "Shifts where in its cycle the LFO starts.");
    knob(ui, setter, &l.smooth, c.at(1), Kn::S, "SMOOTH", color, "Rounds off sharp corners (Square, Saw, Random) so they do not click.");
}

// ---------------------------------------------------------------------------------------------
// Pattern sequencer
// ---------------------------------------------------------------------------------------------

/// Vertical bar that sets a step's wobble depth by dragging up and down.
fn depth_bar(ui: &mut Ui, setter: &ParamSetter, p: &FloatParam, rect: Rect, color: Color32, lit: bool) {
    let resp = ui.interact(rect, Id::new(("depth", p as *const FloatParam as usize)), Sense::click_and_drag());
    if resp.drag_started() {
        setter.begin_set_parameter(p);
    }
    if resp.dragged() || resp.drag_started() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let v = ((rect.bottom() - 3.0 - pos.y) / (rect.height() - 6.0)).clamp(0.0, 1.0);
            setter.set_parameter_normalized(p, v);
        }
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(p);
    }
    if resp.double_clicked() {
        setter.begin_set_parameter(p);
        setter.set_parameter_normalized(p, p.default_normalized_value());
        setter.end_set_parameter(p);
    }
    let v = p.unmodulated_normalized_value();
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, INSET);
    painter.rect_stroke(rect, 4.0, st(1.0, if lit { color } else { LINE }), StrokeKind::Inside);
    let h = (rect.height() - 6.0) * v;
    let bar = Rect::from_min_max(pos2(rect.left() + 3.0, rect.bottom() - 3.0 - h), pos2(rect.right() - 3.0, rect.bottom() - 3.0));
    painter.rect_filled(bar, 2.0, color.gamma_multiply(if lit { 1.0 } else { 0.55 }));
    painter.text(
        pos2(rect.center().x, rect.top() + 10.0),
        Align2::CENTER_CENTER,
        format!("{:.0}%", v * 100.0),
        FontId::proportional(8.5),
        if resp.hovered() || resp.dragged() { TEXT } else { DIM },
    );
    resp.on_hover_ui(|ui| {
        ui.set_max_width(260.0);
        ui.label(egui::RichText::new(format!("Depth   {:.0} %", v * 100.0)).color(TEXT).size(12.5));
        ui.label(egui::RichText::new("How strong the wobble is on this step. 0% holds the filter still.").color(DIM).size(11.5));
        ui.label(egui::RichText::new("Drag up/down  ·  Double-click = reset").color(FAINT).size(10.5));
    });
}

fn pattern_panel(ui: &mut Ui, setter: &ParamSetter, p: &WubParams, current_step: i32, rect: Rect) {
    panel(ui, rect, "PATTERN", LIME);
    let (l, t) = (rect.left(), rect.top());
    let pat = &p.pattern;
    power(ui, setter, &pat.on, pos2(l + 96.0, t + 13.5), LIME, "Switch the pattern on. While it is on, each step sets how fast LFO 1 wobbles, replacing LFO 1's own rate.");
    caption(ui, pos2(l + 122.0, t + 13.5), "STEP");
    dropdown(ui, setter, &pat.step, Rect::from_min_size(pos2(l + 152.0, t + 4.0), vec2(70.0, 19.0)), "How long each step lasts.");
    caption(ui, pos2(l + 236.0, t + 13.5), "LENGTH");
    let len = pat.len.value();
    if button(ui, Rect::from_min_size(pos2(l + 284.0, t + 4.0), vec2(20.0, 19.0)), "len-", "-", TEXT, "One step shorter.") && len > 1 {
        setter.begin_set_parameter(&pat.len);
        setter.set_parameter(&pat.len, len - 1);
        setter.end_set_parameter(&pat.len);
    }
    ui.painter().text(pos2(l + 318.0, t + 13.5), Align2::CENTER_CENTER, format!("{len}"), FontId::proportional(12.0), TEXT);
    if button(ui, Rect::from_min_size(pos2(l + 332.0, t + 4.0), vec2(20.0, 19.0)), "len+", "+", TEXT, "One step longer.") && (len as usize) < STEPS {
        setter.begin_set_parameter(&pat.len);
        setter.set_parameter(&pat.len, len + 1);
        setter.end_set_parameter(&pat.len);
    }
    ui.painter().text(
        pos2(rect.right() - 12.0, t + 13.5),
        Align2::RIGHT_CENTER,
        "Each step: wobble depth (bar), play / rest, wobble speed and a pitch offset in semitones.",
        FontId::proportional(9.5),
        FAINT,
    );

    let len = pat.len.value() as usize;
    let on = pat.on.value();
    let cw = (rect.width() - 20.0) / STEPS as f32;
    for (i, s) in pat.steps.iter().enumerate() {
        let x = l + 10.0 + i as f32 * cw;
        let w = cw - 6.0;
        let active = i < len;
        let playing = on && current_step == i as i32;
        let plays = s.on.value();

        let head = Rect::from_min_size(pos2(x, t + 32.0), vec2(w, 14.0));
        ui.painter().rect_filled(head, 3.0, if playing { LIME } else { RAISED });
        ui.painter().text(
            head.center(),
            Align2::CENTER_CENTER,
            format!("{}", i + 1),
            FontId::proportional(9.0),
            if playing { Color32::BLACK } else if active { DIM } else { FAINT },
        );
        depth_bar(ui, setter, &s.depth, Rect::from_min_size(pos2(x, t + 50.0), vec2(w, 88.0)), if plays { LIME } else { FAINT }, playing);
        toggle(ui, setter, &s.on, Rect::from_min_size(pos2(x, t + 143.0), vec2(w, 17.0)), if plays { "PLAY" } else { "REST" }, LIME, "PLAY: this step sounds. REST: this step is silent.");
        dropdown(ui, setter, &s.rate, Rect::from_min_size(pos2(x, t + 165.0), vec2(w, 19.0)), "Wobble speed during this step, as a note length.");
        hslider(ui, setter, &s.semi, Rect::from_min_size(pos2(x, t + 189.0), vec2(w, 18.0)), MAGENTA, "Pitch offset for this step in semitones (12 = one octave up). GLIDE slides between steps.");
        if !active {
            let cover = Rect::from_min_max(pos2(x - 2.0, t + 30.0), pos2(x + w + 2.0, t + 210.0));
            ui.painter().rect_filled(cover, 3.0, PANEL.gamma_multiply(0.72));
        }
    }
}
