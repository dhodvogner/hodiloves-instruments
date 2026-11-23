//! Step 4: how it plays. One part at a time (chords, bass, drums, melody): pick how it plays and
//! what it sounds like, with a plain-words explanation of every option.

use super::*;
use crate::drums::{fill_key, pattern_for, PATTERNS};
use crate::synth::sounds_for;

const PART_NAMES: [&str; 4] = ["Chords", "Bass", "Drums", "Melody"];
const PART_COLORS: [Color32; 4] = [MAJOR_C, MINOR_C, AMBER, AQUA];

fn flag(cx: &Cx, part: usize) -> (bool, f32) {
    match part {
        0 => (cx.p.chords_on.value(), cx.p.chords_vol.value()),
        1 => (cx.p.bass_on.value(), cx.p.bass_vol.value()),
        2 => (cx.p.drums_on.value(), cx.p.drums_vol.value()),
        _ => (cx.p.melody_on.value(), cx.p.melody_vol.value()),
    }
}

fn summary(cx: &Cx, part: usize) -> String {
    match part {
        0 => format!("{}  ·  {}", cx.cfg.chord_style.name(), sounds_for(0).0[cx.p.sound.value().max(0) as usize % sounds_for(0).0.len()]),
        1 => format!("{}  ·  {}", cx.cfg.bass_style.name(), sounds_for(1).0[cx.p.bass_sound.value().max(0) as usize % sounds_for(1).0.len()]),
        2 => match cx.cfg.drum_pattern {
            Some(i) => PATTERNS[i].name.to_string(),
            None => format!("Auto: {}", pattern_for(cx.genre, 0, None).name),
        },
        _ => sounds_for(2).0[cx.p.melody_sound.value().max(0) as usize % sounds_for(2).0.len()].to_string(),
    }
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    label(ui, pos2(area.left(), area.top() + 14.0), Align2::LEFT_CENTER, "How should it play?", 26.0, TEXT);
    label(ui, pos2(area.left(), area.top() + 46.0), Align2::LEFT_CENTER, "Pick a part, then choose how it plays and what it sounds like. Press Play (top right) to hear every change.", 13.5, DIM);

    // The four parts.
    let gap = 12.0;
    let tw = (area.width() - 3.0 * gap) / 4.0;
    for i in 0..4 {
        let r = Rect::from_min_size(pos2(area.left() + i as f32 * (tw + gap), area.top() + 78.0), vec2(tw, 64.0));
        let (on, _) = flag(cx, i);
        let selected = st.sound_part == i;
        let resp = ui.interact(r, Id::new(("part", i)), Sense::click());
        let col = PART_COLORS[i];
        ui.painter().rect_filled(r, 10.0, if selected { col.gamma_multiply(0.2) } else if resp.hovered() { RAISED_HOVER } else { RAISED });
        ui.painter().rect_stroke(r, 10.0, st_line(selected, col), StrokeKind::Inside);
        ui.painter().circle_filled(pos2(r.left() + 20.0, r.top() + 22.0), 5.0, if on { col } else { FAINT });
        label(ui, pos2(r.left() + 34.0, r.top() + 22.0), Align2::LEFT_CENTER, PART_NAMES[i], 16.0, if on { TEXT } else { DIM });
        let clip = ui.painter_at(r.shrink2(vec2(6.0, 0.0)));
        clip.text(pos2(r.left() + 20.0, r.top() + 45.0), Align2::LEFT_CENTER, if on { summary(cx, i) } else { "off".to_string() }, FontId::proportional(11.5), DIM);
        if resp.clicked() {
            st.sound_part = i;
        }
    }

    let top = area.top() + 160.0;
    let h = 330.0;
    let w1 = 320.0;
    let w3 = 372.0;
    let c1 = Rect::from_min_size(pos2(area.left(), top), vec2(w1, h));
    let c2 = Rect::from_min_size(pos2(c1.right() + 14.0, top), vec2(w1, h));
    let c3 = Rect::from_min_size(pos2(c2.right() + 14.0, top), vec2(area.right() - c2.right() - 14.0, h));
    let _ = w3;
    let part = st.sound_part;
    let color = PART_COLORS[part];
    #[allow(unused_assignments)]
    let mut hover_text: Option<(String, String)> = None;

    // Column 1: how it plays (or the sound, for the melody).
    card(ui, c1);
    match part {
        0 => {
            caps(ui, c1.left_top() + vec2(20.0, 24.0), "HOW IT PLAYS");
            let names: Vec<&str> = CHORD_STYLES.iter().map(|s| s.name()).collect();
            let cur = cx.p.chord_style.value().max(0) as usize;
            let (pick, hov) = radio_list(ui, Rect::from_min_size(c1.left_top() + vec2(14.0, 44.0), vec2(w1 - 28.0, 7.0 * 36.0)), "cstyle", &names, cur, color);
            if let Some(i) = pick {
                set(cx.setter, &cx.p.chord_style, i as i32);
            }
            hover_text = hov.map(|i| (CHORD_STYLES[i].name().to_string(), CHORD_STYLES[i].blurb().to_string()));
        }
        1 => {
            caps(ui, c1.left_top() + vec2(20.0, 24.0), "HOW IT PLAYS");
            let names: Vec<&str> = BASS_STYLES.iter().map(|s| s.name()).collect();
            let cur = cx.p.bass_style.value().max(0) as usize;
            let (pick, hov) = radio_list(ui, Rect::from_min_size(c1.left_top() + vec2(14.0, 44.0), vec2(w1 - 28.0, 7.0 * 36.0)), "bstyle", &names, cur, color);
            if let Some(i) = pick {
                set(cx.setter, &cx.p.bass_style, i as i32);
            }
            hover_text = hov.map(|i| (BASS_STYLES[i].name().to_string(), BASS_STYLES[i].blurb().to_string()));
        }
        2 => {
            caps(ui, c1.left_top() + vec2(20.0, 24.0), "PATTERN");
            let auto = format!("Auto ({})", cx.genre.name());
            let mut names: Vec<&str> = vec![auto.as_str()];
            names.extend(PATTERNS.iter().map(|p| p.name));
            let cur = cx.p.drum_pattern.value().max(0) as usize;
            let (pick, hov) = radio_list(ui, Rect::from_min_size(c1.left_top() + vec2(14.0, 44.0), vec2(w1 - 28.0, 9.0 * 31.0)), "dpat", &names, cur, color);
            if let Some(i) = pick {
                set(cx.setter, &cx.p.drum_pattern, i as i32);
            }
            hover_text = hov.map(|i| {
                if i == 0 {
                    ("Auto".to_string(), format!("Chooses patterns that suit {} and alternates two of them from section to section.", cx.genre.name()))
                } else {
                    (PATTERNS[i - 1].name.to_string(), PATTERNS[i - 1].blurb.to_string())
                }
            });
        }
        _ => {
            caps(ui, c1.left_top() + vec2(20.0, 24.0), "SOUND");
            let (names, blurbs) = sounds_for(2);
            let cur = cx.p.melody_sound.value().max(0) as usize;
            let (pick, hov) = radio_list(ui, Rect::from_min_size(c1.left_top() + vec2(14.0, 44.0), vec2(w1 - 28.0, names.len() as f32 * 40.0)), "msound", names, cur, color);
            if let Some(i) = pick {
                set(cx.setter, &cx.p.melody_sound, i as i32);
            }
            hover_text = hov.map(|i| (names[i].to_string(), blurbs[i].to_string()));
        }
    }

    // Column 2: what it sounds like (or extras).
    card(ui, c2);
    match part {
        0 | 1 => {
            caps(ui, c2.left_top() + vec2(20.0, 24.0), "SOUND");
            let (names, blurbs) = sounds_for(part as u8);
            let param = if part == 0 { &cx.p.sound } else { &cx.p.bass_sound };
            let cur = param.value().max(0) as usize;
            let (pick, hov) = radio_list(ui, Rect::from_min_size(c2.left_top() + vec2(14.0, 44.0), vec2(w1 - 28.0, names.len() as f32 * 40.0)), "sound", names, cur, color);
            if let Some(i) = pick {
                set(cx.setter, param, i as i32);
            }
            if let Some(i) = hov {
                hover_text = Some((names[i].to_string(), blurbs[i].to_string()));
            }
            if part == 0 {
                // Chord extras below the sound list.
                let y = c2.top() + 44.0 + names.len() as f32 * 40.0 + 14.0;
                if let Some(v) = switch(ui, Rect::from_min_size(pos2(c2.left() + 20.0, y), vec2(w1 - 40.0, 28.0)), "open", "Open voicing (wider chords)", cx.p.spread.value(), color,
                    "The second-highest note drops an octave for a wider, more cinematic sound.") {
                    set(cx.setter, &cx.p.spread, v);
                }
                label(ui, pos2(c2.left() + 20.0, y + 46.0), Align2::LEFT_CENTER, "Octave", 13.0, DIM);
                if let Some(i) = segmented(ui, Rect::from_min_size(pos2(c2.left() + 90.0, y + 30.0), vec2(210.0, 32.0)), "oct", &["−2", "−1", "0", "+1", "+2"], (cx.p.octave.value() + 2).clamp(0, 4) as usize, color) {
                    set(cx.setter, &cx.p.octave, i as i32 - 2);
                }
            }
        }
        2 => {
            caps(ui, c2.left_top() + vec2(20.0, 24.0), "FILLS AND SECTIONS");
            if let Some(v) = switch(ui, Rect::from_min_size(c2.left_top() + vec2(20.0, 44.0), vec2(w1 - 40.0, 30.0)), "fills", "A fill on the last bar of each section", cx.p.drum_fills.value(), color,
                "A short snare roll or tom run that leads into the next section.") {
                set(cx.setter, &cx.p.drum_fills, v);
            }
            caps(ui, c2.left_top() + vec2(20.0, 100.0), "PATTERN PER SECTION");
            for (i, s) in cx.project.sections.iter().take(5).enumerate() {
                let pat = pattern_for(cx.genre, i, cx.cfg.drum_pattern);
                label(ui, c2.left_top() + vec2(20.0, 130.0 + i as f32 * 28.0), Align2::LEFT_CENTER, format!("Section {}", s.name), 13.0, DIM);
                label(ui, c2.left_top() + vec2(120.0, 130.0 + i as f32 * 28.0), Align2::LEFT_CENTER, pat.name, 13.5, AMBER);
            }
            wrapped(ui, Rect::from_min_size(c2.left_top() + vec2(20.0, h - 62.0), vec2(w1 - 40.0, 54.0)),
                &format!("Fill key for Neon Break: {}. The drums here are a built-in kit; in your DAW they go to Neon Break (see Export).", fill_key(cx.genre)), 11.5, FAINT);
        }
        _ => {
            caps(ui, c2.left_top() + vec2(20.0, 24.0), "REGISTER");
            let octs = ["−1", "0", "+1", "+2"];
            label(ui, c2.left_top() + vec2(20.0, 66.0), Align2::LEFT_CENTER, "Octave", 13.0, DIM);
            if let Some(i) = segmented(ui, Rect::from_min_size(c2.left_top() + vec2(90.0, 48.0), vec2(200.0, 34.0)), "moct", &octs, (cx.p.mel_octave.value() + 1).clamp(0, 3) as usize, color) {
                set(cx.setter, &cx.p.mel_octave, i as i32 - 1);
            }
            wrapped(ui, Rect::from_min_size(c2.left_top() + vec2(20.0, 100.0), vec2(w1 - 40.0, 150.0)),
                "Higher or lower for generated melodies. Notes you draw yourself in the Melody step play exactly where you put them.", 12.5, DIM);
        }
    }

    // Column 3: about, level, on/off.
    card(ui, c3);
    let (title, text) = hover_text.unwrap_or_else(|| selected_info(cx, part));
    label(ui, c3.left_top() + vec2(20.0, 30.0), Align2::LEFT_CENTER, title, 18.0, color);
    wrapped(ui, Rect::from_min_size(c3.left_top() + vec2(20.0, 56.0), vec2(c3.width() - 40.0, 120.0)), &text, 13.5, Color32::from_rgb(205, 210, 220));
    let (on, _) = flag(cx, part);
    let by = c3.bottom() - 128.0;
    caps(ui, pos2(c3.left() + 20.0, by), "THIS PART");
    let (on_param, vol_param): (&BoolParam, &FloatParam) = match part {
        0 => (&cx.p.chords_on, &cx.p.chords_vol),
        1 => (&cx.p.bass_on, &cx.p.bass_vol),
        2 => (&cx.p.drums_on, &cx.p.drums_vol),
        _ => (&cx.p.melody_on, &cx.p.melody_vol),
    };
    if let Some(v) = switch(ui, Rect::from_min_size(pos2(c3.left() + 20.0, by + 18.0), vec2(c3.width() - 40.0, 30.0)), "partson", &format!("Play the {}", PART_NAMES[part].to_lowercase()), on, color,
        "Mute or unmute this part (playback, MIDI out and the built-in sound). It is still exported.") {
        set(cx.setter, on_param, v);
    }
    label(ui, pos2(c3.left() + 20.0, by + 76.0), Align2::LEFT_CENTER, "Level", 13.0, DIM);
    hslider(ui, cx.setter, vol_param, Rect::from_min_size(pos2(c3.left() + 80.0, by + 64.0), vec2(c3.width() - 100.0, 24.0)), color, "How loud this part is in the built-in sound.");

    // Playback options.
    let py = top + h + 14.0;
    let pr = Rect::from_min_size(pos2(area.left(), py), vec2(area.width(), 84.0));
    card(ui, pr);
    let (l, t) = (pr.left() + 20.0, pr.top());
    caps(ui, pos2(l, t + 20.0), "BUILT-IN SOUND");
    if let Some(v) = switch(ui, Rect::from_min_size(pos2(l, t + 34.0), vec2(250.0, 30.0)), "internal", "Play the built-in sound", cx.p.internal.value(), ACCENT,
        "Turn off when you route the MIDI to your own instruments.") {
        set(cx.setter, &cx.p.internal, v);
    }
    caps(ui, pos2(l + 290.0, t + 20.0), "TO YOUR DAW");
    if let Some(v) = switch(ui, Rect::from_min_size(pos2(l + 290.0, t + 34.0), vec2(250.0, 30.0)), "midiout", "Send MIDI to the host", cx.p.midi_out.value(), ACCENT,
        "Chords on channel 1, bass on 2, melody on 3 and drums on channel 10 (General MIDI, which Neon Break understands). Route them to your instruments. Hosts differ in how they expose plugin MIDI output; the Export step always works.") {
        set(cx.setter, &cx.p.midi_out, v);
    }
    caps(ui, pos2(l + 590.0, t + 20.0), "PLAY CHORDS FROM SINGLE KEYS");
    if let Some(i) = segmented(ui, Rect::from_min_size(pos2(l + 590.0, t + 30.0), vec2(300.0, 36.0)), "kbd", &["Off", "Triads", "7ths", "9ths"], cx.p.chord_input.value().max(0) as usize, ACCENT) {
        set(cx.setter, &cx.p.chord_input, i as i32);
    }
}

fn st_line(selected: bool, col: Color32) -> egui::Stroke {
    plugin_common::ui::st(if selected { 2.0 } else { 1.0 }, if selected { col } else { LINE })
}

/// Title and description of what is currently chosen for a part.
fn selected_info(cx: &Cx, part: usize) -> (String, String) {
    match part {
        0 => (cx.cfg.chord_style.name().to_string(), cx.cfg.chord_style.blurb().to_string()),
        1 => (cx.cfg.bass_style.name().to_string(), cx.cfg.bass_style.blurb().to_string()),
        2 => {
            let p = pattern_for(cx.genre, 0, cx.cfg.drum_pattern);
            (p.name.to_string(), p.blurb.to_string())
        }
        _ => {
            let (names, blurbs) = sounds_for(2);
            let i = cx.p.melody_sound.value().max(0) as usize % names.len();
            (names[i].to_string(), blurbs[i].to_string())
        }
    }
}
