//! The chord palette: every chord that fits the key, laid out so you can audition and compare
//! them without digging through menus. Green = major-ish, purple = minor-ish, grey = the rest.
//! A filled dot means the chord is fully inside the home key; a hollow dot means it borrows.

use super::*;

const ROWS: [Ext; 6] = [Ext::Triad, Ext::Seventh, Ext::Ninth, Ext::Sus2, Ext::Sus4, Ext::Add9];

/// The key and scale the palette is currently showing.
pub fn source_key(cx: &Cx, source: Source) -> (u8, Scale) {
    match source {
        Source::Home => (cx.key, cx.scale),
        Source::Fifths(n) => (fifths_key(cx.key, n), cx.scale),
        Source::Mode(i) => (cx.key, Scale::from_index(i)),
    }
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let (pkey, pscale) = source_key(cx, st.source);
    let pflats = use_flats(pkey, pscale);
    let title = format!("CHORD PALETTE  ·  {} {}", pc_name(pkey, pflats), pscale.name());
    let inner = panel(ui, rect, &title, MAJOR_C);
    let (l, t) = (inner.left(), inner.top());

    // Source tabs: home key, neighbours on the circle of fifths, parallel modes.
    let home = st.source == Source::Home;
    if pill(ui, Rect::from_min_size(pos2(l, t + 1.0), vec2(74.0, 20.0)), "src-home", "HOME KEY", home, ACCENT,
        "The chords of your key and scale. Start here.") {
        st.source = Source::Home;
    }
    let steps = if let Source::Fifths(n) = st.source { n } else { 0 };
    if button(ui, Rect::from_min_size(pos2(l + 84.0, t + 1.0), vec2(24.0, 20.0)), "fifth-left", "‹", TEXT,
        "Step one place left on the circle of fifths (the subdominant side). Each step is a more distant key: more exotic chords.") {
        st.source = Source::Fifths((steps - 1).max(-5));
    }
    let fifth_label = match st.source {
        Source::Fifths(n) if n != 0 => format!("{} {}  ({:+})", pc_name(pkey, pflats), pscale.name(), n),
        _ => "ADJACENT KEYS".to_string(),
    };
    let active = matches!(st.source, Source::Fifths(n) if n != 0);
    ui.painter().text(pos2(l + 192.0, t + 11.0), Align2::CENTER_CENTER, fifth_label, FontId::proportional(10.0), if active { ACCENT } else { DIM });
    if button(ui, Rect::from_min_size(pos2(l + 276.0, t + 1.0), vec2(24.0, 20.0)), "fifth-right", "›", TEXT,
        "Step one place right on the circle of fifths (the dominant side).") {
        st.source = Source::Fifths((steps + 1).min(5));
    }
    let mode_names: Vec<&str> = SCALES.iter().map(|s| s.name).collect();
    let mode_label = match st.source {
        Source::Mode(i) => SCALES[i].name.to_string(),
        _ => "PARALLEL MODE".to_string(),
    };
    if let Some(i) = dd(ui, Rect::from_min_size(pos2(l + 312.0, t + 1.0), vec2(150.0, 20.0)), "mode", &mode_label, &mode_names,
        if matches!(st.source, Source::Mode(_)) { ACCENT } else { DIM },
        "Borrow chords from another mode on the same tonic (modal interchange): the minor iv in a major key, the major IV in Dorian...") {
        st.source = Source::Mode(i);
    }

    // Grid.
    let label_w = 44.0;
    let top = t + 26.0;
    let cell_w = (inner.width() - label_w) / 7.0;
    let cell_h = 26.0;
    // Column headers: the numerals of the palette's own scale.
    for d in 0..7 {
        let triad = diatonic_chord(pkey, pscale, d, Ext::Triad);
        let x = l + label_w + d as f32 * cell_w;
        let f = function_of(pkey, pscale, &triad);
        ui.painter().text(pos2(x + cell_w / 2.0, top + 5.0), Align2::CENTER_CENTER, roman(pkey, pscale, &triad), FontId::proportional(10.5), DIM);
        ui.painter().circle_filled(pos2(x + cell_w / 2.0 - 16.0, top + 5.0), 2.0, function_color(f));
    }
    for (ri, ext) in ROWS.iter().enumerate() {
        let y = top + 13.0 + ri as f32 * (cell_h + 3.0);
        ui.painter().text(pos2(l + 2.0, y + cell_h / 2.0), Align2::LEFT_CENTER, ext.label().to_uppercase(), FontId::proportional(8.5), FAINT);
        for d in 0..7 {
            let chord = diatonic_chord(pkey, pscale, d, *ext);
            let x = l + label_w + d as f32 * cell_w;
            let cell = Rect::from_min_size(pos2(x + 1.5, y), vec2(cell_w - 3.0, cell_h));
            let selected = st.sel.and_then(|s| cx.project.item(s)).map_or(false, |it| it.chord == chord);
            let rn = roman(cx.key, cx.scale, &chord);
            let resp = chord_cell(ui, cell, Id::new(("pal", ri, d)), &chord, pflats, Some(&rn), chord.in_key(cx.key, cx.scale), selected);
            let resp = chord_tip(resp, &chord, cx.key, cx.scale, pflats);
            if st.chip(cx, &resp, chord) {
                cx.add_chord(st, chord);
            }
        }
    }
    ui.painter().text(pos2(rect.right() - 14.0, rect.top() + 13.0), Align2::RIGHT_CENTER, "click = listen · double-click = add · drag to the sequence", FontId::proportional(8.5), FAINT);
}
