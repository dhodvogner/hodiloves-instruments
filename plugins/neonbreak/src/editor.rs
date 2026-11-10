//! egui editor for Neon Break.

use crate::bank::{Slot, PATTERN_KEY, SLOTS};
use crate::params::*;
use crate::presets::{self, PRESETS};
use crate::seq::{note_name, FILL_KEYS, HAT_C, HAT_O, PATTERNS};
use crate::Shared;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui::{self, pos2, vec2, Align2, Color32, FontId, Id, Rect, Sense, StrokeKind, Ui};
use plugin_common::midi::{is_standalone, MIDI_STATUS};
use plugin_common::ui::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// Kick, snare and clap: the backbone of the beat.
const SUNSET: Color32 = Color32::from_rgb(255, 122, 82);
/// Hats and cymbal.
const AQUA: Color32 = Color32::from_rgb(84, 226, 206);
/// Toms, fills and the reverb.
const VIOLET: Color32 = Color32::from_rgb(182, 128, 255);

fn voice_color(index: usize) -> Color32 {
    match index {
        0..=2 => SUNSET,
        3 | 4 | 7 => AQUA,
        _ => VIOLET,
    }
}

pub fn create(params: Arc<NeonParams>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        (),
        |ctx, _| apply_style(ctx, SUNSET),
        move |ctx, setter, _| {
            // Keep the playhead and the pads moving.
            ctx.request_repaint_after(Duration::from_millis(33));
            let p = &*params;
            // Sample paths restored from a project or preset file are read in the background.
            if p.samples.take_pending() {
                let samples = p.samples.clone();
                std::thread::spawn(move || samples.load_all());
            }
            egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ctx, |ui| {
                let o = ui.max_rect().min.to_vec2();
                let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y) + o, vec2(w, h));

                header(ui, setter, p, &shared, r(0.0, 0.0, 1160.0, 60.0));

                let pitch = (1136.0 + 6.0) / VOICES as f32;
                for (i, v) in p.voices().iter().enumerate() {
                    let hit = shared.hits[i].load(Ordering::Relaxed);
                    voice_panel(ui, setter, p, v, i, hit, &shared, r(12.0 + i as f32 * pitch, 64.0, pitch - 6.0, 206.0));
                }

                pattern_panel(ui, setter, p, &shared, r(12.0, 278.0, 1136.0, 300.0));

                groove_panel(ui, setter, p, r(12.0, 586.0, 230.0, 126.0));
                fill_panel(ui, setter, p, r(250.0, 586.0, 250.0, 126.0));
                reverb_panel(ui, setter, p, r(508.0, 586.0, 300.0, 126.0));
                bus_panel(ui, setter, p, r(816.0, 586.0, 332.0, 126.0));

                if is_standalone() {
                    keyboard(ui, r(12.0, 720.0, 1136.0, 62.0), &shared.ui_midi, &shared.keys, SUNSET);
                }
            });
        },
    )
}

fn set<P: Param>(setter: &ParamSetter, p: &P, value: P::Plain) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, value);
    setter.end_set_parameter(p);
}

/// "C5 (MIDI 60)": the name FL Studio's piano roll shows, plus the number other hosts may need.
fn key_text(note: u8) -> String {
    format!("{} (MIDI note {note})", note_name(note))
}

/// A pill-shaped switch that is not a parameter. Returns true when clicked.
fn flag(ui: &mut Ui, rect: Rect, id: &str, label: &str, on: bool, color: Color32, tip: &str) -> bool {
    let resp = ui.interact(rect, Id::new(("flag", id)), Sense::click());
    let painter = ui.painter();
    let radius = rect.height() * 0.5;
    if on {
        painter.rect_filled(rect, radius, color.gamma_multiply(0.18));
        painter.rect_stroke(rect, radius, st(1.0, color.gamma_multiply(0.8)), StrokeKind::Inside);
    } else {
        painter.rect_filled(rect, radius, if resp.hovered() { RAISED_HOVER } else { RAISED });
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(9.5), if on { color } else { DIM });
    let clicked = resp.clicked();
    resp.on_hover_text(tip);
    clicked
}

// ---------------------------------------------------------------------------------------------
// Header: kit browser, preset files, output
// ---------------------------------------------------------------------------------------------

fn header(ui: &mut Ui, setter: &ParamSetter, p: &NeonParams, shared: &Shared, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    ui.painter().text(pos2(l + 18.0, t + 25.0), Align2::LEFT_CENTER, "NEON BREAK", FontId::proportional(25.0), TEXT);
    spaced_text(ui.painter(), pos2(l + 19.0, t + 46.0), "DRUM MACHINE", 8.5, SUNSET, 1.6);

    let list: Vec<(&str, &str)> = PRESETS.iter().map(|x| (x.name, x.category)).collect();
    let current = (p.preset.load(Ordering::Relaxed).max(0) as usize).min(PRESETS.len() - 1);
    if let Some(loaded) = shared.io.take_loaded() {
        plugin_common::preset_io::apply_values(p, setter, &loaded.values);
        if let Some(extra) = &loaded.extra {
            presets::apply_extra(p, extra);
        }
        *shared.io.current.lock().unwrap() = Some(loaded.name);
    }
    let user = shared.io.current.lock().unwrap().clone();
    if let Some(i) = preset_bar(ui, Rect::from_min_size(pos2(l + 300.0, t + 15.0), vec2(300.0, 30.0)), &list, current, user.as_deref(), SUNSET) {
        presets::apply(p, setter, &PRESETS[i]);
        p.preset.store(i as i32, Ordering::Relaxed);
        *shared.io.current.lock().unwrap() = None;
    }
    if button(ui, Rect::from_min_size(pos2(l + 606.0, t + 17.0), vec2(46.0, 26.0)), "preset-load", "LOAD", DIM, "Load a preset file saved with SAVE: the kit, all twelve pattern slots and the samples.") {
        shared.io.import("Neon Break");
    }
    if button(ui, Rect::from_min_size(pos2(l + 656.0, t + 17.0), vec2(46.0, 26.0)), "preset-save", "SAVE", DIM, "Save everything to a preset file: the kit, all twelve pattern slots and which sample files are loaded (the files themselves stay where they are).") {
        let name = user.clone().unwrap_or_else(|| PRESETS[current].name.to_string());
        shared.io.export_with("Neon Break", &name, Some(p as &dyn Params), Some(presets::extra_json(p)));
    }
    if button(ui, Rect::from_min_size(pos2(l + 708.0, t + 17.0), vec2(52.0, 26.0)), "reset", "RESET", DIM, "Reset the drum sounds and effects to the default kit. Your patterns and samples stay.") {
        presets::apply(p, setter, &PRESETS[0]);
        p.preset.store(0, Ordering::Relaxed);
        *shared.io.current.lock().unwrap() = None;
    }

    meter(ui, Rect::from_min_size(pos2(l + 940.0, t + 21.0), vec2(148.0, 10.0)), shared.meter.load(Ordering::Relaxed), SUNSET);
    // One status line for preset files, pattern files and sample loading.
    if let Some(message) = p.samples.take_status() {
        let now = ui.input(|i| i.time);
        ui.data_mut(|d| d.insert_temp(Id::new("preset-io-status"), (message, now)));
    }
    let first = io_status(ui, &shared.io);
    let message = io_status(ui, &shared.pattern_io).or(first);
    let status = message.as_deref().or(MIDI_STATUS.get().map(|s| s.as_str())).unwrap_or("OUTPUT");
    let clip = Rect::from_min_size(pos2(l + 800.0, t + 34.0), vec2(288.0, 14.0));
    ui.painter_at(clip).text(clip.right_center(), Align2::RIGHT_CENTER, status, FontId::proportional(8.5), FAINT);
    knob(ui, setter, &p.gain, pos2(l + 1124.0, t + 26.0), Kn::S, "OUTPUT", SUNSET, "Overall output volume. A soft limiter after it catches peaks; if the meter is red, turn this down.");
}

// ---------------------------------------------------------------------------------------------
// One drum
// ---------------------------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn voice_panel(ui: &mut Ui, setter: &ParamSetter, p: &NeonParams, v: &VoiceParams, index: usize, hit: f32, shared: &Shared, rect: Rect) {
    let info = &VOICE_INFO[index];
    let color = voice_color(index);
    panel(ui, rect, info.label, color);
    let (l, t) = (rect.left(), rect.top());

    // Audition pad: shows the drum's key, lights up on every hit, click to play the drum.
    let pad = Rect::from_min_size(pos2(rect.right() - 62.0, t + 5.0), vec2(30.0, 17.0));
    let resp = ui.interact(pad, Id::new(("pad", index)), Sense::click());
    if resp.clicked() {
        shared.pads.push(0x90, info.note, 110);
    }
    let glow = (hit * 1.3).clamp(0.0, 1.0);
    ui.painter().rect_filled(pad, 8.5, if resp.hovered() { RAISED_HOVER } else { RAISED });
    ui.painter().rect_filled(pad, 8.5, color.gamma_multiply(0.85 * glow));
    ui.painter().text(pad.center(), Align2::CENTER_CENTER, note_name(info.note), FontId::proportional(9.5), if glow > 0.4 { Color32::BLACK } else { TEXT });
    resp.on_hover_text(format!(
        "Click to hear the {}. It lights up whenever the drum plays.\nIn the piano roll, a note on {} plays one hit; the note's velocity sets how hard.",
        info.name.to_lowercase(),
        key_text(info.note)
    ));
    toggle(ui, setter, &v.mute, Rect::from_min_size(pos2(rect.right() - 28.0, t + 5.0), vec2(18.0, 17.0)), "M", HOT, "Mute: silences this drum without changing its pattern.");

    let cols = [l + 26.0, l + rect.width() * 0.5, rect.right() - 26.0];
    let rows = [t + 50.0, t + 98.0, t + 146.0];
    knob(ui, setter, &v.level, pos2(cols[0], rows[0]), Kn::S, "LEVEL", color, "Volume of this drum in the kit.");
    knob(ui, setter, &v.pan, pos2(cols[1], rows[0]), Kn::S, "PAN", color, "Position between the left and right speaker.");
    knob(ui, setter, &v.send, pos2(cols[2], rows[0]), Kn::S, "REVERB", VIOLET, "How much of this drum goes into the gated reverb. Every hit that is sent also opens the gate.");
    knob(ui, setter, &v.tune, pos2(cols[0], rows[1]), Kn::S, "TUNE", color, "Pitch in semitones, for the synthesized drum and the sample alike. Tune the kick and toms to the key of your track.");
    knob(ui, setter, &v.decay, pos2(cols[1], rows[1]), Kn::S, "DECAY", color, "How long the hit rings. Keep it short at drum and bass tempo so hits do not smear into each other. With a sample, 100 % plays the whole file untouched.");
    knob(ui, setter, &v.tone, pos2(cols[2], rows[1]), Kn::S, "TONE", color, &format!("{} (Synthesized drum only.)", info.tone_tip));
    knob(ui, setter, &v.a, pos2(cols[0], rows[2]), Kn::S, info.a.0, color, &format!("{} (Synthesized drum only.)", info.a.1));
    knob(ui, setter, &v.b, pos2(cols[1], rows[2]), Kn::S, info.b.0, color, &format!("{} (Synthesized drum only.)", info.b.1));
    knob(ui, setter, &v.sample_mix, pos2(cols[2], rows[2]), Kn::S, "SAMPLE", AQUA, "Blend between the synthesized drum (0 %) and the loaded sample (100 %). In between, both play together: for example a sampled snare on top of a synthesized body. Does nothing until a sample is loaded.");

    // Sample slot: click to load a WAV file, right-click to remove it.
    let slot = Rect::from_min_size(pos2(l + 8.0, t + 178.0), vec2(rect.width() - 16.0, 20.0));
    let resp = ui.interact(slot, Id::new(("sample", index)), Sense::click());
    let name = p.samples.name(index);
    if resp.clicked() {
        let samples = p.samples.clone();
        plugin_common::preset_io::pick_file("Audio sample (WAV, OGG, WV)", &["wav", "WAV", "ogg", "OGG", "wv", "WV"], move |path| {
            samples.set(index, &path.to_string_lossy());
        });
    }
    if resp.secondary_clicked() && name.is_some() {
        p.samples.set(index, "");
    }
    // Drag and drop: drop a WAV file anywhere on the drum's panel.
    let (hovering, dropped, pointer) = ui.ctx().input(|i| {
        (
            !i.raw.hovered_files.is_empty(),
            i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>(),
            i.pointer.latest_pos(),
        )
    });
    let over_panel = pointer.is_some_and(|p| rect.contains(p));
    if over_panel {
        // Any file is tried: the loader recognises the format and reports what it cannot read.
        // Loaded on a background thread so a big file never stalls the window.
        if let Some(path) = dropped.first() {
            let samples = p.samples.clone();
            let path = path.to_string_lossy().to_string();
            std::thread::spawn(move || samples.set(index, &path));
        }
    }
    let drop_target = hovering && over_panel;
    let painter = ui.painter();
    if hovering {
        // While a file is dragged over the plugin: every drum is a possible target (thin outline),
        // the one under the pointer is the active one (thick outline and tint).
        if drop_target {
            painter.rect_filled(rect, 8.0, AQUA.gamma_multiply(0.16));
            painter.rect_stroke(rect.shrink(1.0), 8.0, st(3.0, AQUA), StrokeKind::Inside);
        } else {
            painter.rect_stroke(rect.shrink(1.0), 8.0, st(1.0, AQUA.gamma_multiply(0.45)), StrokeKind::Inside);
        }
    }
    painter.rect_filled(slot, 5.0, if resp.hovered() || drop_target { RAISED_HOVER } else { INSET });
    painter.rect_stroke(slot, 5.0, st(1.0, if name.is_some() { AQUA.gamma_multiply(0.6) } else { LINE }), StrokeKind::Inside);
    let clip = slot.shrink2(vec2(6.0, 0.0));
    let (text, text_color) = match &name {
        Some(_) if drop_target => ("release to replace", AQUA),
        None if drop_target => ("release to load", AQUA),
        Some(n) => (n.as_str(), TEXT),
        None => ("+ load sample", FAINT),
    };
    ui.painter_at(clip).text(clip.center(), Align2::CENTER_CENTER, text, FontId::proportional(9.5), text_color);
    resp.on_hover_text(format!(
        "{}\nClick to load a WAV file for the {}, or drag one from Explorer onto this drum. Right-click to remove it and go back to the synthesized drum.\nThe file is not copied: keep it where it is, or the drum falls back to the synth.",
        name.as_deref().map(|n| format!("Sample: {n}")).unwrap_or_else(|| "No sample: this drum is synthesized.".to_string()),
        info.name.to_lowercase()
    ));
}

// ---------------------------------------------------------------------------------------------
// Pattern
// ---------------------------------------------------------------------------------------------

/// One step of one drum. The bar height is the velocity; a dimmed bar plays only some of the time.
/// Edits `vel` and `prob` in place.
#[allow(clippy::too_many_arguments)]
fn step_cell(ui: &mut Ui, vel: &mut f32, prob: &mut f32, id: (usize, usize), rect: Rect, color: Color32, playing: bool, downbeat: bool) {
    let resp = ui.interact(rect, Id::new(("step", id)), Sense::click_and_drag());
    let shift = ui.input(|i| i.modifiers.shift);
    if resp.secondary_clicked() || (resp.clicked() && shift) {
        // Cycle the chance: 100 -> 75 -> 50 -> 25 -> 100 %.
        *prob = if *prob > 0.9 { 0.75 } else if *prob > 0.6 { 0.5 } else if *prob > 0.4 { 0.25 } else { 1.0 };
    } else if resp.clicked() {
        *vel = if *vel > 0.005 { 0.0 } else { 0.85 };
    }
    if resp.dragged() {
        *vel = (*vel - resp.drag_delta().y * 0.012).clamp(0.0, 1.0);
    }

    let on = *vel > 0.005;
    let painter = ui.painter();
    let fill = if resp.hovered() { RAISED_HOVER } else if downbeat { RAISED } else { INSET };
    painter.rect_filled(rect, 4.0, fill);
    if on {
        let inner = rect.shrink(2.0);
        let h = (inner.height() * (0.25 + 0.75 * *vel)).max(3.0);
        let bar = Rect::from_min_max(pos2(inner.left(), inner.bottom() - h), inner.max);
        painter.rect_filled(bar, 3.0, color.gamma_multiply(0.35 + 0.65 * *prob));
        if *prob < 0.9 {
            painter.text(rect.center(), Align2::CENTER_CENTER, format!("{:.0}", *prob * 100.0), FontId::proportional(8.5), Color32::BLACK);
        }
    }
    if playing {
        painter.rect_stroke(rect, 4.0, st(1.5, if on { TEXT } else { DIM }), StrokeKind::Inside);
    }
    let (v, c) = (*vel, *prob);
    resp.on_hover_ui(|ui| {
        ui.set_max_width(270.0);
        let state = if on { format!("velocity {:.0} %, chance {:.0} %", v * 100.0, c * 100.0) } else { "off".to_string() };
        ui.label(egui::RichText::new(format!("Step {}   {state}", id.1 + 1)).color(TEXT).size(12.5));
        ui.label(egui::RichText::new("Bar height = how hard the drum is hit. Low bars are ghost notes: quieter, shorter and duller, they fill the gaps between the main hits. A step with less than 100 % chance only plays some of the time, so the loop keeps changing.").color(DIM).size(11.5));
        ui.label(egui::RichText::new("Click = on / off  ·  Drag up/down = velocity  ·  Right-click = chance").color(FAINT).size(10.5));
    });
}

/// The pattern library: a button that opens a list. Returns the pattern that was picked.
fn pattern_menu(ui: &mut Ui, rect: Rect) -> Option<&'static crate::seq::Pattern> {
    let id = Id::new("pattern-menu");
    let popup_id = id.with("popup");
    let resp = ui.interact(rect, id, Sense::click());
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, if resp.hovered() { RAISED_HOVER } else { RAISED });
    painter.text(pos2(rect.left() + 9.0, rect.center().y), Align2::LEFT_CENTER, "LIBRARY", FontId::proportional(10.0), TEXT);
    painter.text(pos2(rect.right() - 9.0, rect.center().y), Align2::RIGHT_CENTER, "v", FontId::proportional(9.0), DIM);
    let mut picked = None;
    egui::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(190.0);
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            let mut last = "";
            for pattern in PATTERNS {
                if pattern.group != last {
                    if !last.is_empty() {
                        ui.add_space(4.0);
                    }
                    ui.label(egui::RichText::new(pattern.group.to_uppercase()).size(9.5).color(SUNSET));
                    last = pattern.group;
                }
                if ui.selectable_label(false, egui::RichText::new(pattern.name).size(12.0)).clicked() {
                    picked = Some(pattern);
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        });
    });
    resp.on_hover_text("Ready-made beats. Picking one puts it into the selected slot, replacing what was there.\nThe Synthwave ones switch HALF-TIME on.");
    picked
}

fn pattern_panel(ui: &mut Ui, setter: &ParamSetter, p: &NeonParams, shared: &Shared, rect: Rect) {
    panel(ui, rect, "PATTERN", SUNSET);
    let (l, t) = (rect.left(), rect.top());

    // The slot being edited. Everything below works on a copy that is written back if it changed.
    let index = (p.slot.value() - 1).clamp(0, SLOTS as i32 - 1) as usize;
    let before = p.bank.slot(index);
    let mut slot = before.clone();
    if let Some(loaded) = shared.pattern_io.take_loaded() {
        if let Some(new) = loaded.extra.as_ref().and_then(|extra| presets::slot_from_json(&loaded.name, extra)) {
            slot = new;
        }
    }

    // ---- Row 1: when patterns play, and tools for the selected slot
    dropdown(ui, setter, &p.mode, Rect::from_min_size(pos2(l + 98.0, t + 4.0), vec2(118.0, 19.0)), "When the patterns play.\nPattern keys: only while a pattern key is held, so the arrangement is written as notes in the piano roll.\nAlways on: the selected slot plays whenever the DAW plays; pattern keys still switch to other slots.\nOff: no patterns, only the drum notes you send.");
    let playing = shared.play.load(Ordering::Relaxed);
    let play_tip = if is_standalone() {
        "Start or stop the selected slot. The speed is set with TEMPO."
    } else {
        "Plays the selected slot on its own clock, so you can hear a beat while editing it without holding a key. Switch it off again before playing your song."
    };
    if button(ui, Rect::from_min_size(pos2(l + 222.0, t + 4.0), vec2(50.0, 19.0)), "play", if playing { "STOP" } else { "PLAY" }, if playing { SUNSET } else { TEXT }, play_tip) {
        shared.play.store(!playing, Ordering::Relaxed);
    }
    if flag(ui, Rect::from_min_size(pos2(l + 284.0, t + 5.0), vec2(82.0, 17.0)), "half", "HALF-TIME", slot.grid.half, SUNSET, "This slot plays at half speed: its sixteen steps last two bars. A drum and bass two-step turns into a slow, heavy backbeat with the snare on beat 3. Keep a full-speed and a half-time slot side by side and switch between them with their pattern keys.") {
        slot.grid.half = !slot.grid.half;
    }
    if flag(ui, Rect::from_min_size(pos2(l + 372.0, t + 5.0), vec2(80.0, 17.0)), "keep", "KEEP HATS", slot.grid.keep_hats, AQUA, "In half-time, keep both hi-hats of this slot running at full speed. The beat slows down but the energy stays up.") {
        slot.grid.keep_hats = !slot.grid.keep_hats;
    }
    if let Some(pattern) = pattern_menu(ui, Rect::from_min_size(pos2(l + 464.0, t + 4.0), vec2(96.0, 19.0))) {
        slot = Slot::from_pattern(pattern);
    }
    if button(ui, Rect::from_min_size(pos2(l + 566.0, t + 4.0), vec2(50.0, 19.0)), "clear", "CLEAR", DIM, "Empty the selected slot, to start a beat from nothing.") {
        slot = Slot::empty();
    }
    if button(ui, Rect::from_min_size(pos2(l + 622.0, t + 4.0), vec2(46.0, 19.0)), "pattern-load", "LOAD", DIM, "Load a pattern file into the selected slot.") {
        shared.pattern_io.import("Neon Break Pattern");
    }
    if button(ui, Rect::from_min_size(pos2(l + 672.0, t + 4.0), vec2(46.0, 19.0)), "pattern-save", "SAVE", DIM, "Save the selected slot as a pattern file. The file name becomes the pattern's name. (All twelve slots are also saved with your project and in preset files.)") {
        shared.pattern_io.export_with("Neon Break Pattern", &slot.name, None, Some(presets::slot_json(&slot)));
    }
    ui.painter().text(
        pos2(rect.right() - 12.0, t + 13.5),
        Align2::RIGHT_CENTER,
        "click a step = on / off  ·  drag = velocity  ·  right-click = chance",
        FontId::proportional(9.5),
        FAINT,
    );

    // ---- Row 2: the twelve slots, each with the piano-roll key that plays it
    let names = p.bank.names();
    let now_playing = shared.playing_slot.load(Ordering::Relaxed);
    let gap = 4.0;
    let tw = (rect.width() - 20.0 - gap * (SLOTS - 1) as f32) / SLOTS as f32;
    for i in 0..SLOTS {
        let tab = Rect::from_min_size(pos2(l + 10.0 + i as f32 * (tw + gap), t + 30.0), vec2(tw, 22.0));
        let resp = ui.interact(tab, Id::new(("slot", i)), Sense::click());
        if resp.clicked() && i != index {
            set(setter, &p.slot, i as i32 + 1);
        }
        let selected = i == index;
        let sounding = now_playing == i as i32;
        let painter = ui.painter();
        painter.rect_filled(tab, 5.0, if selected { SUNSET.gamma_multiply(0.2) } else if resp.hovered() { RAISED_HOVER } else { RAISED });
        if selected {
            painter.rect_stroke(tab, 5.0, st(1.0, SUNSET), StrokeKind::Inside);
        }
        let key = PATTERN_KEY + i as u8;
        let key_color = if sounding { Color32::BLACK } else if selected { SUNSET } else { DIM };
        let badge = Rect::from_min_size(pos2(tab.left() + 3.0, tab.top() + 3.0), vec2(26.0, 16.0));
        if sounding {
            painter.rect_filled(badge, 4.0, SUNSET);
        }
        painter.text(badge.center(), Align2::CENTER_CENTER, note_name(key), FontId::proportional(9.0), key_color);
        let clip = Rect::from_min_max(pos2(badge.right() + 4.0, tab.top()), pos2(tab.right() - 4.0, tab.bottom()));
        let name = if selected { slot.name.as_str() } else { names.get(i).map(|s| s.as_str()).unwrap_or("") };
        ui.painter_at(clip).text(clip.left_center(), Align2::LEFT_CENTER, name, FontId::proportional(9.5), if selected { TEXT } else { DIM });
        resp.on_hover_text(format!(
            "Slot {}: {name}\nHold {} in the piano roll and this pattern plays for as long as the note lasts. Draw one long note per section of your song; a different key switches to a different pattern.\nClick to show and edit this slot.",
            i + 1,
            key_text(key)
        ));
    }

    // ---- The step grid of the selected slot
    let tick = shared.step.load(Ordering::Relaxed);
    let shown_plays = now_playing == index as i32;
    let label_w = 70.0;
    let group_gap = 6.0;
    let x0 = l + 10.0 + label_w;
    let cw = (rect.right() - 10.0 - x0 - 3.0 * group_gap) / STEPS as f32;
    let cell_x = |i: usize| x0 + i as f32 * cw + (i / 4) as f32 * group_gap;

    for i in 0..STEPS {
        let c = pos2(cell_x(i) + (cw - 3.0) * 0.5, t + 61.0);
        ui.painter().text(c, Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(9.0), if i % 4 == 0 { DIM } else { FAINT });
    }
    for (v, voice) in p.voices().iter().enumerate() {
        let y = t + 70.0 + v as f32 * 27.5;
        let color = voice_color(v);
        let hit = shared.hits[v].load(Ordering::Relaxed);
        let muted = voice.mute.value();
        ui.painter().circle_filled(pos2(l + 17.0, y + 12.0), 3.0, color.gamma_multiply(0.25 + 0.75 * (hit * 1.3).min(1.0)));
        ui.painter().text(pos2(l + 26.0, y + 12.0), Align2::LEFT_CENTER, VOICE_INFO[v].label, FontId::proportional(9.5), if muted { FAINT } else { TEXT });
        let slow = slot.grid.half && !(slot.grid.keep_hats && (v == HAT_C || v == HAT_O));
        let current = if tick < 0 || !shown_plays { -1 } else if slow { (tick / 2) % STEPS as i32 } else { tick % STEPS as i32 };
        for i in 0..STEPS {
            let cell = Rect::from_min_size(pos2(cell_x(i), y), vec2(cw - 3.0, 24.0));
            let c = if muted { FAINT } else { color };
            let (mut vel, mut prob) = (slot.grid.vel[v][i], slot.grid.prob[v][i]);
            step_cell(ui, &mut vel, &mut prob, (v, i), cell, c, current == i as i32, (i / 4) % 2 == 0);
            slot.grid.vel[v][i] = vel;
            slot.grid.prob[v][i] = prob;
        }
    }

    if slot != before {
        // An edited library pattern is no longer that pattern.
        if slot.name == before.name && slot.grid.vel != before.grid.vel && PATTERNS.iter().any(|x| x.name == slot.name) {
            slot.name = format!("{} *", slot.name);
        }
        p.bank.set_slot(index, slot);
    }
}

// ---------------------------------------------------------------------------------------------
// Groove, fills, reverb, bus
// ---------------------------------------------------------------------------------------------

fn groove_panel(ui: &mut Ui, setter: &ParamSetter, p: &NeonParams, rect: Rect) {
    panel(ui, rect, "GROOVE", AQUA);
    let (l, t) = (rect.left(), rect.top());
    let row = Row::new(l + 42.0, t + 66.0, 72.0);
    knob(ui, setter, &p.swing, row.at(0), Kn::M, "SWING", AQUA, "Delays every second step so the beat shuffles. 10-25 % loosens a break; 100 % is a full triplet feel.");
    knob(ui, setter, &p.human, row.at(1), Kn::M, "HUMAN", AQUA, "Makes every hit slightly different, like a real drummer: how hard it is, exactly when it lands, and its pitch and tone (most audible on snares and hats). The kick always stays on the grid.");
    if is_standalone() {
        knob(ui, setter, &p.bpm, row.at(2), Kn::M, "TEMPO", SUNSET, "Speed of the pattern in beats per minute. 174 is drum and bass; a half-time slot feels like 87.");
    } else {
        ui.painter().text(pos2(row.at(2).x, t + 60.0), Align2::CENTER_CENTER, "TEMPO", FontId::proportional(9.5), DIM);
        ui.painter().text(pos2(row.at(2).x, t + 76.0), Align2::CENTER_CENTER, "from DAW", FontId::proportional(10.5), FAINT);
    }
}

/// A button that holds a fill for as long as it is pressed.
#[allow(clippy::too_many_arguments)]
fn fill_button(ui: &mut Ui, setter: &ParamSetter, p: &EnumParam<Fill>, fill: Fill, rect: Rect, label: &str, key: u8, tip: &str) {
    let id = Id::new(("fill", label));
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let down = resp.is_pointer_button_down_on();
    let was_down: bool = ui.data(|d| d.get_temp(id)).unwrap_or(false);
    if down && !was_down {
        setter.begin_set_parameter(p);
        setter.set_parameter(p, fill);
    } else if !down && was_down {
        setter.set_parameter(p, Fill::Off);
        setter.end_set_parameter(p);
    }
    ui.data_mut(|d| d.insert_temp(id, down));

    let active = p.value() == fill;
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 6.0, VIOLET.gamma_multiply(0.3));
        painter.rect_stroke(rect, 6.0, st(1.0, VIOLET), StrokeKind::Inside);
    } else {
        painter.rect_filled(rect, 6.0, if resp.hovered() { RAISED_HOVER } else { RAISED });
    }
    painter.text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, label, FontId::proportional(10.5), if active { VIOLET } else { TEXT });
    painter.text(pos2(rect.right() - 9.0, rect.center().y), Align2::RIGHT_CENTER, note_name(key), FontId::proportional(9.5), if active { VIOLET } else { DIM });
    resp.on_hover_text(format!(
        "{tip}\nIn the piano roll, hold {} for as long as the fill should last: usually the last bar (or half bar) before a new section. While it is held the pattern stops and only the fill plays; it builds up towards the end of the bar.\nHolding this button does the same, for trying it out.",
        key_text(key)
    ));
}

fn fill_panel(ui: &mut Ui, setter: &ParamSetter, p: &NeonParams, rect: Rect) {
    panel(ui, rect, "FILLS", VIOLET);
    let (l, t) = (rect.left(), rect.top());
    ui.painter().text(pos2(rect.right() - 12.0, t + 13.5), Align2::RIGHT_CENTER, "replace the pattern while held", FontId::proportional(9.5), FAINT);
    let w = (rect.width() - 30.0) * 0.5;
    let cell = |col: usize, row: usize| Rect::from_min_size(pos2(l + 10.0 + col as f32 * (w + 10.0), t + 36.0 + row as f32 * 40.0), vec2(w, 32.0));
    fill_button(ui, setter, &p.fill, Fill::SnareRush, cell(0, 0), "SNARE RUSH", FILL_KEYS[0], "A snare roll that gets louder and doubles in speed at the end of the bar.");
    fill_button(ui, setter, &p.fill, Fill::TomFill, cell(1, 0), "TOM FILL", FILL_KEYS[1], "A run down the two toms: the classic 80s fill.");
    fill_button(ui, setter, &p.fill, Fill::HatRoll, cell(0, 1), "HAT ROLL", FILL_KEYS[2], "A fast closed hi-hat roll that speeds up into triplets.");
    fill_button(ui, setter, &p.fill, Fill::Build, cell(1, 1), "BUILD", FILL_KEYS[3], "A riser: steady kicks and a snare roll that accelerates through the whole bar.");
}

fn reverb_panel(ui: &mut Ui, setter: &ParamSetter, p: &NeonParams, rect: Rect) {
    panel(ui, rect, "GATED REVERB", VIOLET);
    let (l, t) = (rect.left(), rect.top());
    caption(ui, pos2(rect.right() - 152.0, t + 13.5), "GATE");
    dropdown(ui, setter, &p.rev_gate, Rect::from_min_size(pos2(rect.right() - 116.0, t + 4.0), vec2(104.0, 19.0)), "How long the reverb stays open after a hit before it is cut off dead - the 80s snare sound. It follows the tempo, so it stays clean at any speed, and doubles for half-time slots. 'Open' is a normal reverb that rings out.");
    let row = Row::new(l + 52.0, t + 66.0, 98.0);
    knob(ui, setter, &p.rev_size, row.at(0), Kn::M, "SIZE", VIOLET, "Size of the room. With the gate on, bigger means a denser, louder burst before the cut.");
    knob(ui, setter, &p.rev_tone, row.at(1), Kn::M, "TONE", VIOLET, "Brightness of the reverb: low is dark and warm, high is bright and splashy.");
    knob(ui, setter, &p.rev_level, row.at(2), Kn::M, "LEVEL", VIOLET, "Volume of the reverb. Each drum's REVERB knob sets how much of it goes in.");
}

fn bus_panel(ui: &mut Ui, setter: &ParamSetter, p: &NeonParams, rect: Rect) {
    panel(ui, rect, "DRUM BUS", SUNSET);
    let (l, t) = (rect.left(), rect.top());
    caption(ui, pos2(rect.right() - 178.0, t + 13.5), "KICK OUT");
    dropdown(ui, setter, &p.kick_out, Rect::from_min_size(pos2(rect.right() - 116.0, t + 4.0), vec2(104.0, 19.0)), "Neon Break has a second stereo output that carries only the kick, for sidechaining a bass to the kick alone.\nIn mix: the kick is part of the drum mix; the second output is silent.\nMix + copy: the same, plus a copy of the kick on the second output. Route that output to a muted mixer track and use it as the sidechain source.\nSeparate: the kick leaves the drum mix and plays only on the second output, so it has its own mixer track (it then skips the drum bus).\nIn FL Studio: open the plugin's wrapper settings (gear icon), Processing tab, and switch on the second output.");
    let row = Row::new(l + 48.0, t + 66.0, 79.0);
    knob(ui, setter, &p.punch, row.at(0), Kn::M, "PUNCH", SUNSET, "Emphasises the first milliseconds of every hit, so the drums cut through basses and pads.");
    knob(ui, setter, &p.drive, row.at(1), Kn::M, "DRIVE", SUNSET, "Saturation over the whole kit: adds grit and weight, and makes the drums sound like one sampled break.");
    knob(ui, setter, &p.glue, row.at(2), Kn::M, "GLUE", SUNSET, "Compression over the whole kit: pulls quiet hits and tails up, so the beat sounds dense and loud.");
    knob(ui, setter, &p.duck, row.at(3), Kn::M, "DUCK", AQUA, "Every kick briefly pushes the other drums and the reverb down, the 'pumping' effect. This is inside Neon Break only; to duck a bass, use KICK OUT.");
}
