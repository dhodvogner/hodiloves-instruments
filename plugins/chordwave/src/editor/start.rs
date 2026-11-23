//! Step 1: what are we making? Genre, key, mood, and a first progression.

use super::*;

const TAGS: [&str; 8] = [
    "Open-minded, no bias",
    "Neon minor-key loops",
    "Fast, rolling, bass-led",
    "Smooth and soulful",
    "Catchy four-chord hooks",
    "Slow, wide, emotional",
    "Jazzy and relaxed",
    "Euphoric and driving",
];

/// (scale index, name, feeling)
const MOODS: [(Scale, &str, &str); 5] = [
    (Scale::Minor, "Minor", "dark, warm"),
    (Scale::Major, "Major", "bright, happy"),
    (Scale::Dorian, "Dorian", "soulful, jazzy"),
    (Scale::HarmonicMinor, "Harmonic minor", "dramatic"),
    (Scale::Lydian, "Lydian", "dreamy"),
];

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    let (l, t, w) = (area.left(), area.top(), area.width());
    label(ui, pos2(l, t + 14.0), Align2::LEFT_CENTER, "What are we making?", 26.0, TEXT);
    label(ui, pos2(l, t + 46.0), Align2::LEFT_CENTER, "Three quick choices. You can change all of them later.", 13.5, DIM);

    // Genre.
    let mut y = t + 92.0;
    caps(ui, pos2(l, y), "1   GENRE");
    y += 20.0;
    let gap = 12.0;
    let gw = (w - 3.0 * gap) / 4.0;
    for (i, g) in GENRES.iter().enumerate() {
        let r = Rect::from_min_size(pos2(l + (i % 4) as f32 * (gw + gap), y + (i / 4) as f32 * 66.0), vec2(gw, 56.0));
        if choice(ui, r, &format!("genre{i}"), g.name(), TAGS[i], *g == cx.genre, ACCENT, "") {
            cx.apply_genre(i);
        }
    }

    // Key.
    y += 2.0 * 66.0 + 18.0;
    caps(ui, pos2(l, y), "2   KEY");
    y += 20.0;
    let kw = (w - 11.0 * 6.0) / 12.0;
    for k in 0..12u8 {
        let r = Rect::from_min_size(pos2(l + k as f32 * (kw + 6.0), y), vec2(kw, 44.0));
        if choice(ui, r, &format!("key{k}"), pc_name(k, use_flats(k, cx.scale)), "", k == cx.key, ACCENT, "") {
            cx.change_key(k);
        }
    }

    // Mood.
    y += 44.0 + 18.0;
    caps(ui, pos2(l, y), "3   MOOD");
    y += 20.0;
    let mw = (w - 5.0 * 10.0) / 6.0;
    for (i, (scale, name, feel)) in MOODS.iter().enumerate() {
        let r = Rect::from_min_size(pos2(l + i as f32 * (mw + 10.0), y), vec2(mw, 56.0));
        if choice(ui, r, &format!("mood{i}"), name, feel, *scale == cx.scale, ACCENT, &scale.def().blurb.to_string()) {
            set(cx.setter, &cx.p.scale, scale.index() as i32);
        }
    }
    let others: Vec<&str> = SCALES.iter().filter(|d| !MOODS.iter().any(|m| m.0 == d.scale)).map(|d| d.name).collect();
    let other_on = !MOODS.iter().any(|m| m.0 == cx.scale);
    let more_rect = Rect::from_min_size(pos2(l + 5.0 * (mw + 10.0), y), vec2(mw, 56.0));
    if let Some(i) = dd(ui, more_rect, "more-scales", if other_on { cx.scale.name() } else { "More scales" }, &others, if other_on { ACCENT } else { DIM },
        "Phrygian, Mixolydian, Locrian, melodic minor and more.") {
        if let Some(d) = SCALES.iter().find(|d| d.name == others[i]) {
            set(cx.setter, &cx.p.scale, d.scale.index() as i32);
        }
    }

    // First progression.
    y += 56.0 + 22.0;
    caps(ui, pos2(l, y), "START FROM A PROGRESSION");
    if big_button(ui, Rect::from_min_size(pos2(area.right() - 230.0, y - 12.0), vec2(230.0, 28.0)), "imp", "or import a MIDI file…", Btn::Quiet,
        "Load a melody, bass line or chords from a MIDI file and build around it.") {
        export::import_dialog(cx);
    }
    y += 22.0;
    let mut list = templates_for(cx.genre);
    list.sort_by_key(|tpl| tpl.scale != cx.scale);
    let cw = (w - 3.0 * gap) / 4.0;
    for (i, tpl) in list.iter().take(4).enumerate() {
        let r = Rect::from_min_size(pos2(l + i as f32 * (cw + gap), y), vec2(cw, 86.0));
        let names: Vec<String> = tpl.chords(cx.key).iter().map(|c| c.name(use_flats(cx.key, tpl.scale))).collect();
        let resp = ui.interact(r, Id::new(("tpl", i)), Sense::click());
        let hot = resp.hovered();
        ui.painter().rect_filled(r, 10.0, if hot { RAISED_HOVER } else { RAISED });
        ui.painter().rect_stroke(r, 10.0, plugin_common::ui::st(1.0, LINE), StrokeKind::Inside);
        label(ui, pos2(r.left() + 14.0, r.top() + 22.0), Align2::LEFT_CENTER, tpl.name, 15.0, TEXT);
        let clip = ui.painter_at(r.shrink(2.0));
        clip.text(pos2(r.left() + 14.0, r.top() + 46.0), Align2::LEFT_CENTER, names.join("   "), FontId::proportional(13.5), AQUA);
        let note = if tpl.scale != cx.scale { format!("switches to {}", tpl.scale.name()) } else { "click to use".to_string() };
        clip.text(pos2(r.left() + 14.0, r.top() + 68.0), Align2::LEFT_CENTER, note, FontId::proportional(11.0), FAINT);
        resp.clone().on_hover_text(tpl.why);
        if resp.clicked() {
            cx.apply_template(st, tpl);
            cx.flash(st, format!("Loaded \"{}\". Press Play to listen, then go on to Chords.", tpl.name));
        }
    }
    if let Some((text, at)) = st.message.clone() {
        if cx.time - at < 8.0 {
            label(ui, pos2(l, area.bottom() - 6.0), Align2::LEFT_CENTER, text, 13.0, AQUA);
        }
    }
}
