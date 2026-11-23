//! Step 4: how it plays. Chords, bass and drums as plain choices.

use super::*;
use crate::drums::{fill_key, pattern_for};

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    label(ui, pos2(area.left(), area.top() + 14.0), Align2::LEFT_CENTER, "How should it play?", 26.0, TEXT);
    label(ui, pos2(area.left(), area.top() + 46.0), Align2::LEFT_CENTER, "Press Play (top right) and try the options. The change is heard right away.", 13.5, DIM);

    let top = area.top() + 82.0;
    let gap = 16.0;
    let cw = (area.width() - 2.0 * gap) / 3.0;
    let ch = 360.0;
    let c1 = Rect::from_min_size(pos2(area.left(), top), vec2(cw, ch));
    let c2 = Rect::from_min_size(pos2(c1.right() + gap, top), vec2(cw, ch));
    let c3 = Rect::from_min_size(pos2(c2.right() + gap, top), vec2(cw, ch));

    // Chords.
    part_header(ui, cx, c1, "Chords", MAJOR_C, &cx.p.chords_on);
    let names: Vec<&str> = CHORD_STYLES.iter().map(|s| s.name()).collect();
    let cur = cx.p.chord_style.value().max(0) as usize;
    if let Some(i) = radio_list(ui, Rect::from_min_size(c1.left_top() + vec2(16.0, 62.0), vec2(cw - 32.0, 7.0 * 30.0)), "cstyle", &names, cur, MAJOR_C) {
        set(cx.setter, &cx.p.chord_style, i as i32);
    }
    if let Some(v) = switch(ui, Rect::from_min_size(c1.left_top() + vec2(20.0, 290.0), vec2(180.0, 30.0)), "open", "Open voicing", cx.p.spread.value(), MAJOR_C,
        "The second-highest note drops an octave for a wider, more cinematic sound.") {
        set(cx.setter, &cx.p.spread, v);
    }
    let octs = ["−2", "−1", "0", "+1", "+2"];
    label(ui, c1.left_top() + vec2(20.0, 336.0), Align2::LEFT_CENTER, "Octave", 13.0, DIM);
    if let Some(i) = segmented(ui, Rect::from_min_size(c1.left_top() + vec2(100.0, 318.0), vec2(220.0, 34.0)), "oct", &octs, (cx.p.octave.value() + 2).clamp(0, 4) as usize, MAJOR_C) {
        set(cx.setter, &cx.p.octave, i as i32 - 2);
    }

    // Bass.
    part_header(ui, cx, c2, "Bass", MINOR_C, &cx.p.bass_on);
    let names: Vec<&str> = BASS_STYLES.iter().map(|s| s.name()).collect();
    let cur = cx.p.bass_style.value().max(0) as usize;
    if let Some(i) = radio_list(ui, Rect::from_min_size(c2.left_top() + vec2(16.0, 62.0), vec2(cw - 32.0, 7.0 * 30.0)), "bstyle", &names, cur, MINOR_C) {
        set(cx.setter, &cx.p.bass_style, i as i32);
    }
    wrapped(ui, Rect::from_min_size(c2.left_top() + vec2(20.0, 290.0), vec2(cw - 40.0, 48.0)),
        "The bass follows your chords. Drum and bass patterns sit lower than synthwave ones.", 12.0, FAINT);

    // Drums.
    card(ui, c3);
    label(ui, c3.left_top() + vec2(20.0, 28.0), Align2::LEFT_CENTER, "Drums", 17.0, TEXT);
    ui.painter().circle_filled(c3.left_top() + vec2(cw - 28.0, 28.0), 5.0, AMBER);
    wrapped(ui, Rect::from_min_size(c3.left_top() + vec2(20.0, 56.0), vec2(cw - 40.0, 70.0)),
        "Drums are made for Neon Break and exported with the other parts (the built-in sound here plays chords, bass and melody).", 12.5, DIM);
    caps(ui, c3.left_top() + vec2(20.0, 150.0), "PATTERN PER SECTION");
    for (i, s) in cx.project.sections.iter().take(4).enumerate() {
        let pat = pattern_for(cx.genre, i);
        label(ui, c3.left_top() + vec2(20.0, 180.0 + i as f32 * 28.0), Align2::LEFT_CENTER, format!("Section {}", s.name), 13.0, DIM);
        label(ui, c3.left_top() + vec2(130.0, 180.0 + i as f32 * 28.0), Align2::LEFT_CENTER, pat.name, 13.5, AMBER);
    }
    label(ui, c3.left_top() + vec2(20.0, ch - 70.0), Align2::LEFT_CENTER, format!("Fill on the last bar of each section (key {})", fill_key(cx.genre)), 12.0, FAINT);
    let _ = st;

    // Playback options.
    let py = top + ch + 16.0;
    let pr = Rect::from_min_size(pos2(area.left(), py), vec2(area.width(), 104.0));
    card(ui, pr);
    let (l, t) = (pr.left() + 20.0, pr.top());
    caps(ui, pos2(l, t + 22.0), "BUILT-IN SOUND");
    let sounds: Vec<&str> = crate::synth::SOUND_NAMES.to_vec();
    if let Some(i) = segmented(ui, Rect::from_min_size(pos2(l, t + 40.0), vec2(250.0, 36.0)), "sound", &sounds, cx.p.sound.value().max(0) as usize, ACCENT) {
        set(cx.setter, &cx.p.sound, i as i32);
    }
    if let Some(v) = switch(ui, Rect::from_min_size(pos2(l + 290.0, t + 42.0), vec2(220.0, 32.0)), "internal", "Play the built-in sound", cx.p.internal.value(), ACCENT,
        "Turn off when you route the MIDI to your own instruments.") {
        set(cx.setter, &cx.p.internal, v);
    }
    if let Some(v) = switch(ui, Rect::from_min_size(pos2(l + 530.0, t + 42.0), vec2(220.0, 32.0)), "midiout", "Send MIDI to the host", cx.p.midi_out.value(), ACCENT,
        "Chords on channel 1, bass on 2, melody on 3: route them to any instrument. Hosts differ in how they expose plugin MIDI output; the Export step always works.") {
        set(cx.setter, &cx.p.midi_out, v);
    }
    caps(ui, pos2(l + 780.0, t + 22.0), "KEYBOARD");
    let names = crate::params::input_names();
    let short = ["Normal", "Chords", "7ths", "9ths"];
    let _ = names;
    if let Some(i) = segmented(ui, Rect::from_min_size(pos2(l + 780.0, t + 40.0), vec2(220.0, 36.0)), "kbd", &short, cx.p.chord_input.value().max(0) as usize, ACCENT) {
        set(cx.setter, &cx.p.chord_input, i as i32);
    }
}

fn part_header(ui: &mut Ui, cx: &Cx, r: Rect, title: &str, color: Color32, on: &BoolParam) {
    card(ui, r);
    label(ui, r.left_top() + vec2(20.0, 28.0), Align2::LEFT_CENTER, title, 17.0, TEXT);
    if let Some(v) = switch(ui, Rect::from_min_size(r.right_top() + vec2(-72.0, 12.0), vec2(60.0, 32.0)), &format!("on-{title}"), "", on.value(), color, "Mute or unmute this part (playback, MIDI out and the built-in sound).") {
        set(cx.setter, on, v);
    }
}
