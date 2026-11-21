//! The idea panel: what could come next, which chords fit a melody, and ready-made progressions.
//! Every suggestion says why it works, so the panel teaches as it suggests.

use super::*;
use crate::suggest::*;

const ROW_H: f32 = 36.0;

/// Wheel scrolling for a clipped list. Returns the new offset.
pub fn scroll(ui: &Ui, rect: Rect, offset: f32, content_h: f32) -> f32 {
    let max = (content_h - rect.height()).max(0.0);
    let mut o = offset;
    if ui.rect_contains_pointer(rect) {
        o -= ui.input(|i| i.smooth_scroll_delta.y);
    }
    o.clamp(0.0, max)
}

fn score_bar(ui: &Ui, rect: Rect, score: f32, color: Color32) {
    let p = ui.painter();
    p.rect_filled(rect, 2.0, INSET);
    p.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width() * score.clamp(0.0, 1.0), rect.height())), 2.0, color.gamma_multiply(0.8));
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let inner = panel(ui, rect, "IDEAS", AMBER);
    let (l, t) = (inner.left(), inner.top());
    for (i, (tab, label, tip)) in [
        (RightTab::Next, "NEXT CHORD", "Chords that could follow the selected one, ranked, each with the music theory reason."),
        (RightTab::Match, "MATCH MELODY", "Chords that fit the melody notes (over the selected chord, or all of them)."),
        (RightTab::Progressions, "PROGRESSIONS", "Ready-made progressions for the chosen genre, written in your key."),
    ]
    .iter()
    .enumerate()
    {
        if pill(ui, Rect::from_min_size(pos2(l + i as f32 * 106.0, t + 1.0), vec2(100.0, 20.0)), &format!("tab{i}"), label, st.tab == *tab, AMBER, tip) {
            st.tab = *tab;
        }
    }
    let area = Rect::from_min_max(pos2(l, t + 28.0), inner.max);
    match st.tab {
        RightTab::Next => next(ui, cx, st, area),
        RightTab::Match => matching(ui, cx, st, area),
        RightTab::Progressions => progressions(ui, cx, st, area),
    }
}

fn row_list(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect, slot: usize, rows: &[(Chord, f32, String)], on_double: &dyn Fn(&Cx, &mut State, Chord)) {
    let content = rows.len() as f32 * ROW_H;
    st.list_scroll[slot] = scroll(ui, area, st.list_scroll[slot], content);
    let off = st.list_scroll[slot];
    for (i, (chord, score, reason)) in rows.iter().enumerate() {
        let y = area.top() + i as f32 * ROW_H - off;
        if y + ROW_H < area.top() || y > area.bottom() {
            continue;
        }
        let cell = Rect::from_min_size(pos2(area.left(), y + 3.0), vec2(80.0, ROW_H - 8.0));
        let visible = cell.intersect(area);
        if visible.height() < 10.0 {
            continue;
        }
        let rn = roman(cx.key, cx.scale, chord);
        let resp = chord_cell(ui, cell, Id::new(("idea", slot, i)), chord, cx.flats, Some(&rn), chord.in_key(cx.key, cx.scale), false);
        let resp = chord_tip(resp, chord, cx.key, cx.scale, cx.flats);
        if st.chip(cx, &resp, *chord) {
            on_double(cx, st, *chord);
        }
        score_bar(ui, Rect::from_min_size(pos2(area.left() + 88.0, y + 15.0), vec2(46.0, 5.0)), *score, AMBER);
        let text_rect = Rect::from_min_size(pos2(area.left() + 142.0, y + 2.0), vec2(area.width() - 146.0, ROW_H - 4.0));
        wrapped(ui, text_rect.intersect(area), reason, 9.5, DIM);
    }
}

fn next(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    let placed = cx.project.placed();
    let prev = st.sel.and_then(|s| cx.project.item(s)).map(|i| i.chord).or_else(|| placed.last().map(|p| p.chord));
    let head = match &prev {
        Some(c) => format!("After {} ({}):", c.name(cx.flats), roman(cx.key, cx.scale, c)),
        None => "Good chords to start with:".to_string(),
    };
    ui.painter().text(pos2(area.left() + 2.0, area.top() + 6.0), Align2::LEFT_CENTER, head, FontId::proportional(10.5), TEXT);
    ui.painter().text(pos2(area.right() - 2.0, area.top() + 6.0), Align2::RIGHT_CENTER, "double-click to add after the selection", FontId::proportional(8.5), FAINT);
    let rows: Vec<(Chord, f32, String)> = suggest_next(prev.as_ref(), cx.key, cx.scale, cx.genre).into_iter().map(|s| (s.chord, s.score, s.reason)).collect();
    let list = Rect::from_min_max(pos2(area.left(), area.top() + 16.0), area.max);
    row_list(ui, cx, st, list, 0, &rows, &|cx, st, c| cx.add_chord(st, c));
}

fn matching(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    let placed = cx.project.placed();
    let span = st.sel.and_then(|s| placed.iter().find(|p| p.sel == s)).map(|p| (p.start, p.start + p.item_beats));
    let notes: Vec<Note> = match span {
        Some((a, b)) => cx.project.melody_in(a, b),
        None => cx.project.melody.clone(),
    };
    if cx.project.melody.is_empty() {
        wrapped(ui, Rect::from_min_size(area.min + vec2(4.0, 8.0), vec2(area.width() - 8.0, 80.0)),
            "No melody yet. Draw notes in the piano roll below, import a MIDI file, or press GENERATE: then this tab lists the chords that fit those notes (over the selected chord, or over the whole melody), and can re-harmonise the whole sketch around your melody.", 10.5, DIM);
        return;
    }
    // Header: key guess and re-harmonise.
    let guess = detect_key(&pc_weights(&cx.project.melody));
    if let Some(g) = guess.first() {
        let label = format!("Melody sounds like {} {}", pc_name(g.key, use_flats(g.key, g.scale)), g.scale.name());
        let same = g.key == cx.key && g.scale == cx.scale;
        ui.painter().text(pos2(area.left() + 2.0, area.top() + 8.0), Align2::LEFT_CENTER, label, FontId::proportional(10.5), if same { AQUA } else { AMBER });
        if !same && button(ui, Rect::from_min_size(pos2(area.left() + 200.0, area.top() + 0.0), vec2(80.0, 17.0)), "use-key", "USE THIS KEY", AMBER,
            "Set the key and scale to what the melody suggests (the sketch is not transposed).") {
            set(cx.setter, &cx.p.key, g.key as i32);
            set(cx.setter, &cx.p.scale, g.scale.index() as i32);
        }
    }
    if button(ui, Rect::from_min_size(pos2(area.right() - 112.0, area.top()), vec2(112.0, 17.0)), "reharm", "RE-HARMONISE ALL", ACCENT,
        "Replace every chord with the best match for the melody notes over it (lengths stay). Use undo in your DAW, or CLEAR and rebuild.") {
        reharmonise(cx);
    }
    let weights = pc_weights(&notes);
    let rows: Vec<(Chord, f32, String)> = match_melody(&weights, cx.key, cx.scale, cx.genre).into_iter().take(14).map(|m| (m.chord, m.score, m.reason)).collect();
    let list = Rect::from_min_max(pos2(area.left(), area.top() + 22.0), area.max);
    row_list(ui, cx, st, list, 1, &rows, &|cx, st, c| {
        if let Some(sel) = st.sel {
            cx.p.project.edit(|pr| {
                if let Some(it) = pr.item_mut(sel) {
                    it.chord = c;
                }
            });
        } else {
            cx.add_chord(st, c);
        }
    });
}

fn reharmonise(cx: &Cx) {
    let placed = cx.project.placed();
    let changes: Vec<(Sel, Chord)> = placed
        .iter()
        .filter_map(|p| {
            let notes = cx.project.melody_in(p.start, p.start + p.item_beats);
            if notes.is_empty() {
                return None;
            }
            let best = match_melody(&pc_weights(&notes), cx.key, cx.scale, cx.genre).into_iter().next()?;
            (best.score >= 0.5).then_some((p.sel, best.chord))
        })
        .collect();
    cx.p.project.edit(|pr| {
        for (sel, chord) in &changes {
            if let Some(it) = pr.item_mut(*sel) {
                it.chord = *chord;
            }
        }
    });
}

fn progressions(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    let list = templates_for(cx.genre);
    st.template = st.template.min(list.len().saturating_sub(1));
    let rows_area = Rect::from_min_size(area.min, vec2(area.width(), 4.0 * 26.0));
    let content = list.len() as f32 * 26.0;
    st.list_scroll[2] = scroll(ui, rows_area, st.list_scroll[2], content);
    let off = st.list_scroll[2];
    let clip = ui.painter_at(rows_area);
    for (i, tpl) in list.iter().enumerate() {
        let y = rows_area.top() + i as f32 * 26.0 - off;
        if y + 26.0 < rows_area.top() || y > rows_area.bottom() {
            continue;
        }
        let row = Rect::from_min_size(pos2(rows_area.left(), y), vec2(rows_area.width(), 24.0));
        let resp = ui.interact(row.intersect(rows_area), Id::new(("tpl", i)), Sense::click());
        let selected = i == st.template;
        clip.rect_filled(row, 4.0, if selected { AMBER.gamma_multiply(0.18) } else if resp.hovered() { RAISED_HOVER } else { RAISED });
        clip.text(pos2(row.left() + 8.0, row.center().y), Align2::LEFT_CENTER, tpl.name, FontId::proportional(11.0), if selected { TEXT } else { DIM });
        let names: Vec<String> = tpl.chords(cx.key).iter().map(|c| c.name(use_flats(cx.key, tpl.scale))).collect();
        clip.text(pos2(row.left() + 150.0, row.center().y), Align2::LEFT_CENTER, ellipsize(&names.join("  "), 44), FontId::proportional(10.5), AQUA);
        clip.text(pos2(row.right() - 8.0, row.center().y), Align2::RIGHT_CENTER, format!("{} · {}", tpl.genre.name(), tpl.scale.name()), FontId::proportional(8.5), FAINT);
        if resp.clicked() {
            st.template = i;
        }
    }
    let Some(tpl) = list.get(st.template) else { return };
    let foot = Rect::from_min_max(pos2(area.left(), rows_area.bottom() + 4.0), area.max);
    wrapped(ui, Rect::from_min_size(foot.min, vec2(foot.width(), foot.height() - 26.0)), tpl.why, 9.5, DIM);
    let by = foot.bottom() - 21.0;
    if button(ui, Rect::from_min_size(pos2(foot.left(), by), vec2(130.0, 20.0)), "tpl-add", "ADD AS SECTION", AQUA,
        "Append this progression as a new section, in your key. The scale switches to the progression's own scale.") {
        apply_template(cx, st, tpl, false);
    }
    if button(ui, Rect::from_min_size(pos2(foot.left() + 138.0, by), vec2(130.0, 20.0)), "tpl-replace", "REPLACE CHORDS", ACCENT,
        "Replace all chords with this progression (the melody stays).") {
        apply_template(cx, st, tpl, true);
    }
    ui.painter().text(pos2(foot.right() - 2.0, by + 10.0), Align2::RIGHT_CENTER, "scroll for more", FontId::proportional(8.5), FAINT);
}

fn apply_template(cx: &Cx, st: &mut State, tpl: &Template, replace: bool) {
    if tpl.scale != cx.scale {
        set(cx.setter, &cx.p.scale, tpl.scale.index() as i32);
    }
    let key = cx.key;
    let new_section = cx.p.project.edit(|pr| {
        if replace || pr.is_empty() {
            pr.sections = vec![tpl.section(key, "A")];
            0
        } else {
            let name = ((b'A' + (pr.sections.len() % 26) as u8) as char).to_string();
            pr.sections.push(tpl.section(key, &name));
            pr.sections.len() - 1
        }
    });
    st.cur_section = new_section;
    st.sel = None;
}
