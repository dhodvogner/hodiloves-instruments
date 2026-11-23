//! Step 2: build the chord progression. Your progression on top, seven chords to pick from, ideas
//! for what could come next.

use super::*;
use crate::suggest::*;

const SECTION_COLORS: [Color32; 4] = [Color32::from_rgb(84, 226, 206), Color32::from_rgb(255, 84, 160), Color32::from_rgb(255, 176, 64), Color32::from_rgb(182, 128, 255)];
const RICHNESS: [(Ext, &str); 5] = [(Ext::Triad, "Simple"), (Ext::Seventh, "7ths"), (Ext::Ninth, "9ths"), (Ext::Add9, "Add 9"), (Ext::Sus4, "Sus")];

struct Geo {
    card: Rect,
    lane: Rect,
    ppb: f32,
}

fn geo(area: Rect, beats: f32) -> Geo {
    let card = Rect::from_min_size(pos2(area.left(), area.top() + 78.0), vec2(area.width(), 164.0));
    let lane = Rect::from_min_size(pos2(card.left() + 20.0, card.top() + 62.0), vec2(card.width() - 40.0, 84.0));
    let span = (beats + 4.0).max(16.0);
    Geo { card, lane, ppb: (lane.width() / span).clamp(10.0, 64.0) }
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    label(ui, pos2(area.left(), area.top() + 14.0), Align2::LEFT_CENTER, "Build your chord progression", 26.0, TEXT);
    label(ui, pos2(area.left(), area.top() + 46.0), Align2::LEFT_CENTER, "Click a chord to hear it, press + to add it. Click a block in your progression to change it.", 13.5, DIM);

    let g = geo(area, cx.project.beats());
    progression(ui, cx, st, &g);

    // Edit bar for the selected chord.
    let bar = Rect::from_min_size(pos2(area.left(), g.card.bottom() + 10.0), vec2(area.width(), 42.0));
    edit_bar(ui, cx, st, bar);

    // Lower row: picker and ideas.
    let top = bar.bottom() + 14.0;
    let h = area.bottom() - top;
    let picker = Rect::from_min_size(pos2(area.left(), top), vec2(area.width() * 0.6 - 8.0, h));
    let ideas = Rect::from_min_size(pos2(picker.right() + 16.0, top), vec2(area.right() - picker.right() - 16.0, h));
    picker_card(ui, cx, st, picker);
    ideas_card(ui, cx, st, ideas);
}

// ---------------------------------------------------------------------------------------------
// The progression
// ---------------------------------------------------------------------------------------------

fn progression(ui: &mut Ui, cx: &Cx, st: &mut State, g: &Geo) {
    card(ui, g.card);
    let bars = (cx.project.beats() / BEATS_PER_BAR).ceil() as i32;
    caps(ui, g.card.left_top() + vec2(20.0, 26.0), &format!("YOUR PROGRESSION   ·   {} BAR{}", bars, if bars == 1 { "" } else { "S" }));

    // Section tools, top right.
    let mut x = g.card.right() - 14.0;
    let mut place = |w: f32| {
        x -= w;
        let r = Rect::from_min_size(pos2(x, g.card.top() + 12.0), vec2(w, 28.0));
        x -= 4.0;
        r
    };
    if big_button(ui, place(60.0), "p-clear", "Clear", Btn::Quiet, "Remove every chord (the melody stays).") {
        cx.p.project.edit(|pr| pr.sections = vec![Section { name: "A".into(), items: Vec::new() }]);
        st.sel = None;
        st.cur_section = 0;
    }
    if big_button(ui, place(118.0), "p-delsec", "Delete section", Btn::Quiet, "Delete the highlighted section with its chords and the melody over it.") {
        let si = st.cur_section;
        cx.p.project.edit(|pr| pr.remove_section(si));
        st.sel = None;
    }
    if big_button(ui, place(130.0), "p-dupsec", "Duplicate section", Btn::Quiet, "Copy the highlighted section (chords and melody) right after itself: ready for the next verse or chorus.") {
        let si = st.cur_section;
        cx.p.project.edit(|pr| pr.duplicate_section(si));
        st.cur_section = si + 1;
        st.sel = None;
    }
    if big_button(ui, place(104.0), "p-addsec", "+ Section", Btn::Quiet, "Start a new, empty section. Chords you add go into the highlighted section.") {
        let new = cx.p.project.edit(|pr| pr.add_section());
        st.cur_section = new;
        st.sel = None;
    }
    if big_button(ui, place(130.0), "p-names", "Copy chord names", Btn::Quiet, "Copy the chord names to the clipboard.") {
        let flats = cx.flats;
        let text: Vec<String> = cx.project.sections.iter().map(|s| format!("{}: {}", s.name, s.items.iter().map(|i| i.chord.name(flats)).collect::<Vec<_>>().join(" | "))).collect();
        ui.ctx().copy_text(text.join("\n"));
        cx.flash(st, "Chord names copied");
    }

    let lane = g.lane;
    let end_x = (lane.left() + (cx.project.beats() + 4.0).max(16.0) * g.ppb).min(lane.right());
    ui.painter().rect_filled(Rect::from_min_max(lane.min, pos2(end_x, lane.bottom())), 8.0, INSET);

    let placed = cx.project.placed();
    let playhead = cx.shared.playhead.load(Ordering::Relaxed);

    // Section tabs above the lane.
    let mut start = 0.0;
    for (si, s) in cx.project.sections.iter().enumerate() {
        let beats = s.beats();
        let color = SECTION_COLORS[si % SECTION_COLORS.len()];
        let strip = Rect::from_min_size(pos2(lane.left() + start * g.ppb, lane.top() - 20.0), vec2((beats * g.ppb - 3.0).max(22.0), 17.0));
        let resp = ui.interact(strip, Id::new(("section", si)), Sense::click());
        let active = si == st.cur_section;
        ui.painter().rect_filled(strip, 5.0, color.gamma_multiply(if active { 0.5 } else { 0.18 }));
        ui.painter_at(strip).text(pos2(strip.left() + 8.0, strip.center().y), Align2::LEFT_CENTER, format!("Section {}", s.name), FontId::proportional(10.5), if active { Color32::WHITE } else { color });
        if resp.clicked() {
            st.cur_section = si;
            st.sel = None;
        }
        start += beats;
    }
    if cx.project.sections.get(st.cur_section).is_some_and(|s| s.items.is_empty()) {
        label(ui, pos2(lane.left() + start * g.ppb + 8.0, lane.top() - 11.0), Align2::LEFT_CENTER, "new section: add chords below", 10.5, FAINT);
    }

    // Blocks.
    let mut reorder: Option<(Sel, i32)> = None;
    for (idx, p) in placed.iter().enumerate() {
        let x = lane.left() + p.start * g.ppb;
        let w = (p.item_beats * g.ppb - 4.0).max(10.0);
        let block = Rect::from_min_size(pos2(x + 2.0, lane.top() + 4.0), vec2(w, lane.height() - 8.0));
        let handle = Rect::from_min_max(pos2(block.right() - 10.0, block.top()), block.max);
        let body = Rect::from_min_max(block.min, pos2(block.right() - 10.0, block.bottom()));
        let selected = st.sel == Some(p.sel);
        let col = kind_color(p.chord.kind());
        let now = playhead >= 0.0 && playhead >= p.start && playhead < p.start + p.item_beats;
        let resp = ui.interact(body, Id::new(("blk", idx)), Sense::click_and_drag());
        let hresp = ui.interact(handle, Id::new(("blk-handle", idx)), Sense::drag());
        let painter = ui.painter();
        painter.rect_filled(block, 8.0, col.gamma_multiply(if selected { 0.5 } else if resp.hovered() { 0.32 } else { 0.22 }));
        painter.rect_stroke(block, 8.0, st_stroke(if selected { 2.5 } else { 1.0 }, if selected { ACCENT } else if now { AQUA } else { col.gamma_multiply(0.6) }), StrokeKind::Inside);
        painter.rect_filled(Rect::from_min_max(pos2(block.right() - 6.0, block.top() + 14.0), pos2(block.right() - 4.0, block.bottom() - 14.0)), 1.0, if hresp.hovered() || hresp.dragged() { ACCENT } else { col.gamma_multiply(0.5) });
        let clip = ui.painter_at(body);
        let size = if w > 90.0 { 21.0 } else if w > 56.0 { 15.0 } else { 11.0 };
        clip.text(pos2(block.left() + 12.0, block.center().y - 10.0), Align2::LEFT_CENTER, p.chord.name(cx.flats), FontId::proportional(size), TEXT);
        clip.text(pos2(block.left() + 12.0, block.center().y + 14.0), Align2::LEFT_CENTER, roman(cx.key, cx.scale, &p.chord), FontId::proportional(12.0), DIM);
        if w > 120.0 {
            clip.text(pos2(block.right() - 18.0, block.bottom() - 12.0), Align2::RIGHT_CENTER, format!("{} beats", p.item_beats), FontId::proportional(10.5), FAINT);
        }

        let down = resp.is_pointer_button_down_on();
        match &st.audition {
            None if down => {
                st.sel = Some(p.sel);
                st.cur_section = p.sel.section;
                cx.start_audition(st, resp.id, &p.chord);
            }
            Some((id, _)) if !down && *id == resp.id => cx.stop_audition(st),
            _ => {}
        }
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let left = idx.checked_sub(1).and_then(|i| placed.get(i)).filter(|n| n.sel.section == p.sel.section);
                let right = placed.get(idx + 1).filter(|n| n.sel.section == p.sel.section);
                if let Some(n) = left {
                    if pos.x < lane.left() + (n.start + n.item_beats * 0.5) * g.ppb {
                        reorder = Some((p.sel, -1));
                    }
                }
                if let Some(n) = right {
                    if pos.x > lane.left() + (n.start + n.item_beats * 0.5) * g.ppb {
                        reorder = Some((p.sel, 1));
                    }
                }
            }
        }
        if hresp.dragged() {
            if let Some(pos) = hresp.interact_pointer_pos() {
                let beats = ((pos.x - block.left()) / g.ppb).round().max(1.0);
                let sel = p.sel;
                cx.p.project.edit(|pr| pr.set_beats(sel, beats));
            }
        }
        if hresp.hovered() || hresp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        resp.on_hover_text("Click: hear and select  ·  Drag sideways: reorder  ·  Drag the right edge: make it longer or shorter");
    }
    if let Some((sel, dir)) = reorder {
        let new = cx.p.project.edit(|pr| pr.move_item(sel, dir));
        st.sel = Some(new);
    }
    if playhead >= 0.0 && playhead < cx.project.beats() {
        let x = lane.left() + playhead * g.ppb;
        ui.painter().line_segment([pos2(x, lane.top()), pos2(x, lane.bottom())], st_stroke(2.0, Color32::WHITE));
    }
    if placed.is_empty() {
        label(ui, lane.center(), Align2::CENTER_CENTER, "Empty. Pick a chord below and press + to add it.", 13.5, FAINT);
    }
    if let Some(chord) = st.drag {
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            if lane.expand2(vec2(0.0, 24.0)).contains(pos) {
                let boundary = boundary_near(&placed, ((pos.x - lane.left()) / g.ppb).max(0.0));
                let x = lane.left() + boundary * g.ppb;
                ui.painter().line_segment([pos2(x, lane.top() - 2.0), pos2(x, lane.bottom() + 2.0)], st_stroke(3.0, ACCENT));
            }
        }
        ghost(ui, &chord, cx.flats);
    }
    // Keyboard: delete the selected chord.
    if let Some(sel) = st.sel {
        let delete = ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete && ui.ctx().memory(|m| m.focused().is_none()) {
            cx.p.project.edit(|pr| pr.remove(sel));
            st.sel = None;
        }
    }
}

fn st_stroke(w: f32, c: Color32) -> egui::Stroke {
    plugin_common::ui::st(w, c)
}

fn boundary_near(placed: &[Placed], beat: f32) -> f32 {
    let mut best = 0.0;
    let mut dist = f32::MAX;
    let mut consider = |b: f32| {
        if (b - beat).abs() < dist {
            dist = (b - beat).abs();
            best = b;
        }
    };
    consider(0.0);
    for p in placed {
        consider(p.start);
        consider(p.start + p.item_beats);
    }
    best
}

fn edit_bar(ui: &mut Ui, cx: &Cx, st: &mut State, r: Rect) {
    let Some(sel) = st.sel else {
        label(ui, pos2(r.left() + 4.0, r.center().y), Align2::LEFT_CENTER, "Select a chord in your progression to see what it does and to change it.", 12.5, FAINT);
        return;
    };
    let Some(item) = cx.project.item(sel).cloned() else { return };
    let chord = item.chord;
    let f = function_of(cx.key, cx.scale, &chord);
    label(ui, pos2(r.left() + 4.0, r.top() + 11.0), Align2::LEFT_CENTER, format!("{}   {}", chord.name(cx.flats), roman(cx.key, cx.scale, &chord)), 17.0, TEXT);
    let short = f.explain().split(':').next().unwrap_or("");
    label(ui, pos2(r.left() + 4.0, r.top() + 32.0), Align2::LEFT_CENTER, format!("{}: {}", f.label(), short), 11.5, function_color(f));

    let mut x = r.left() + 400.0;
    let mut slot = |w: f32| {
        let rc = Rect::from_min_size(pos2(x, r.top() + 5.0), vec2(w, 32.0));
        x += w + 6.0;
        rc
    };
    if big_button(ui, slot(36.0), "ch-left", "<", Btn::Normal, "Move this chord one place earlier") {
        let new = cx.p.project.edit(|pr| pr.move_item(sel, -1));
        st.sel = Some(new);
    }
    if big_button(ui, slot(36.0), "ch-right", ">", Btn::Normal, "Move this chord one place later") {
        let new = cx.p.project.edit(|pr| pr.move_item(sel, 1));
        st.sel = Some(new);
    }
    if big_button(ui, slot(40.0), "ch-shorter", "−", Btn::Normal, "One beat shorter") {
        cx.p.project.edit(|pr| pr.set_beats(sel, (item.beats - 1.0).max(0.5)));
    }
    if big_button(ui, slot(40.0), "ch-longer", "+", Btn::Normal, "One beat longer") {
        cx.p.project.edit(|pr| pr.set_beats(sel, item.beats + 1.0));
    }
    let inv_names = ["Voicing: auto", "Root position", "1st inversion", "2nd inversion", "3rd inversion"];
    let inv_label = inv_names[item.inversion.map_or(0, |i| (i as usize + 1).min(4))];
    if let Some(i) = dd(ui, slot(128.0), "ch-inv", inv_label, &inv_names, DIM, "Auto picks the inversion that moves least from the previous chord (smooth voice leading).") {
        cx.p.project.edit(|pr| {
            if let Some(it) = pr.item_mut(sel) {
                it.inversion = if i == 0 { None } else { Some(i as u8 - 1) };
            }
        });
    }
    let mut slash: Vec<String> = vec!["Bass: root".into()];
    slash.extend((0..12u8).map(|p| format!("Bass: {}", pc_name(p, cx.flats))));
    let slash_label = chord.bass.map_or("Bass: root".to_string(), |b| format!("Bass: {}", pc_name(b, cx.flats)));
    if let Some(i) = dd(ui, slot(112.0), "ch-slash", &slash_label, &slash, DIM, "Put another note in the bass (a slash chord such as C/E).") {
        cx.p.project.edit(|pr| {
            if let Some(it) = pr.item_mut(sel) {
                it.chord = it.chord.with_bass(if i == 0 { None } else { Some(i as u8 - 1) });
            }
        });
    }
    if big_button(ui, slot(76.0), "ch-del", "Delete", Btn::Quiet, "Remove this chord (Delete key)") {
        cx.p.project.edit(|pr| pr.remove(sel));
        st.sel = None;
    }
}

// ---------------------------------------------------------------------------------------------
// The picker
// ---------------------------------------------------------------------------------------------

fn source_key(cx: &Cx, source: Source) -> (u8, Scale) {
    match source {
        Source::Home => (cx.key, cx.scale),
        Source::Fifths(n) => (fifths_key(cx.key, n), cx.scale),
        Source::Mode(i) => (cx.key, Scale::from_index(i)),
    }
}

/// A big chord button. Returns the response and whether its + was pressed.
fn tile(ui: &mut Ui, rect: Rect, id: Id, chord: &Chord, flats: bool, roman_text: &str, in_key: bool, selected: bool) -> (Response, bool) {
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let col = kind_color(chord.kind());
    let hot = resp.hovered() || resp.is_pointer_button_down_on();
    let painter = ui.painter();
    painter.rect_filled(rect, 10.0, col.gamma_multiply(if hot { 0.34 } else { 0.18 }));
    painter.rect_stroke(rect, 10.0, st_stroke(if selected { 2.0 } else { 1.0 }, if selected { ACCENT } else { col.gamma_multiply(0.5) }), StrokeKind::Inside);
    let name = chord.name(flats);
    let size = if name.chars().count() <= 4 { 19.0 } else if name.chars().count() <= 6 { 15.5 } else { 12.5 };
    painter.text(pos2(rect.center().x, rect.top() + rect.height() * 0.42), Align2::CENTER_CENTER, name, FontId::proportional(size), TEXT);
    painter.text(pos2(rect.center().x, rect.top() + rect.height() * 0.76), Align2::CENTER_CENTER, roman_text, FontId::proportional(11.5), DIM);
    let dot = pos2(rect.left() + 11.0, rect.top() + 11.0);
    if in_key {
        painter.circle_filled(dot, 3.2, col);
    } else {
        painter.circle_stroke(dot, 3.2, st_stroke(1.3, col));
    }
    // The + button.
    let plus = Rect::from_center_size(pos2(rect.right() - 13.0, rect.top() + 13.0), vec2(22.0, 22.0));
    let presp = ui.interact(plus, id.with("plus"), Sense::click());
    let painter = ui.painter();
    painter.circle_filled(plus.center(), 10.0, if presp.hovered() { ACCENT } else { Color32::from_black_alpha(90) });
    painter.text(plus.center() + vec2(0.0, -0.5), Align2::CENTER_CENTER, "+", FontId::proportional(15.0), Color32::WHITE);
    presp.clone().on_hover_text("Add to your progression");
    (resp, presp.clicked())
}

fn picker_card(ui: &mut Ui, cx: &Cx, st: &mut State, r: Rect) {
    card(ui, r);
    let (l, t) = (r.left() + 20.0, r.top());
    let (pkey, pscale) = source_key(cx, st.source);
    let pflats = use_flats(pkey, pscale);
    caps(ui, pos2(l, t + 26.0), "PICK A CHORD");
    // Richness.
    let sel_i = RICHNESS.iter().position(|(e, _)| *e == st.richness).unwrap_or(0);
    let names: Vec<&str> = RICHNESS.iter().map(|x| x.1).collect();
    if let Some(i) = segmented(ui, Rect::from_min_size(pos2(r.right() - 20.0 - 330.0, t + 12.0), vec2(330.0, 34.0)), "rich", &names, sel_i, ACCENT) {
        st.richness = RICHNESS[i].0;
    }

    // Seven chords.
    let gap = 8.0;
    let tw = (r.width() - 40.0 - 6.0 * gap) / 7.0;
    for d in 0..7 {
        let chord = diatonic_chord(pkey, pscale, d, st.richness);
        let rect = Rect::from_min_size(pos2(l + d as f32 * (tw + gap), t + 64.0), vec2(tw, 92.0));
        let selected = st.sel.and_then(|s| cx.project.item(s)).is_some_and(|it| it.chord == chord);
        let rn = roman(cx.key, cx.scale, &chord);
        let (resp, plus) = tile(ui, rect, Id::new(("tile", d)), &chord, pflats, &rn, chord.in_key(cx.key, cx.scale), selected);
        let resp = chord_tip(resp, &chord, cx.key, cx.scale, pflats);
        if st.chip(cx, &resp, chord) || plus {
            cx.add_chord(st, chord);
        }
    }

    // Where the chords come from.
    let y = t + 176.0;
    label(ui, pos2(l, y + 17.0), Align2::LEFT_CENTER, "Chords from", 13.0, DIM);
    let src_i = match st.source {
        Source::Home => 0,
        Source::Fifths(_) => 1,
        Source::Mode(_) => 2,
    };
    if let Some(i) = segmented(ui, Rect::from_min_size(pos2(l + 100.0, y), vec2(340.0, 34.0)), "src", &["My key", "Neighbour key", "Other mode"], src_i, ACCENT) {
        st.source = match i {
            0 => Source::Home,
            1 => Source::Fifths(1),
            _ => Source::Mode(Scale::Dorian.index()),
        };
    }
    let ex = l + 456.0;
    match st.source {
        Source::Home => label(ui, pos2(ex, y + 17.0), Align2::LEFT_CENTER, "Chords that belong to your key.", 12.0, FAINT),
        Source::Fifths(n) => {
            if big_button(ui, Rect::from_min_size(pos2(ex, y), vec2(34.0, 34.0)), "f-left", "<", Btn::Normal, "A key with one more flat (the subdominant side).") {
                st.source = Source::Fifths((n - 1).max(-5));
            }
            label(ui, pos2(ex + 40.0 + 70.0, y + 17.0), Align2::CENTER_CENTER, format!("{} {}", pc_name(pkey, pflats), pscale.name()), 13.0, ACCENT);
            if big_button(ui, Rect::from_min_size(pos2(ex + 40.0 + 140.0, y), vec2(34.0, 34.0)), "f-right", ">", Btn::Normal, "A key with one more sharp (the dominant side).") {
                st.source = Source::Fifths((n + 1).min(5));
            }
        }
        Source::Mode(i) => {
            let names: Vec<&str> = SCALES.iter().map(|s| s.name).collect();
            if let Some(j) = dd(ui, Rect::from_min_size(pos2(ex, y + 2.0), vec2(190.0, 30.0)), "mode", SCALES[i].name, &names, ACCENT, "Borrow chords from another mode on the same tonic.") {
                st.source = Source::Mode(j);
            }
        }
    }
    // Legend.
    let ly = r.bottom() - 22.0;
    let items = [(MAJOR_C, "major"), (MINOR_C, "minor"), (OTHER_C, "other")];
    let mut lx = l;
    for (c, name) in items {
        ui.painter().circle_filled(pos2(lx + 4.0, ly), 4.0, c);
        label(ui, pos2(lx + 14.0, ly), Align2::LEFT_CENTER, name, 11.5, DIM);
        lx += 74.0;
    }
    ui.painter().circle_filled(pos2(lx + 4.0, ly), 3.2, DIM);
    label(ui, pos2(lx + 14.0, ly), Align2::LEFT_CENTER, "in your key", 11.5, DIM);
    lx += 92.0;
    ui.painter().circle_stroke(pos2(lx + 4.0, ly), 3.2, st_stroke(1.3, DIM));
    label(ui, pos2(lx + 14.0, ly), Align2::LEFT_CENTER, "borrowed from outside", 11.5, DIM);
}

// ---------------------------------------------------------------------------------------------
// Ideas
// ---------------------------------------------------------------------------------------------

fn ideas_card(ui: &mut Ui, cx: &Cx, st: &mut State, r: Rect) {
    card(ui, r);
    let (l, t) = (r.left() + 20.0, r.top());
    let placed = cx.project.placed();
    let prev = st.sel.and_then(|s| cx.project.item(s)).map(|i| i.chord).or_else(|| placed.last().map(|p| p.chord));
    caps(ui, pos2(l, t + 26.0), "WHAT COULD COME NEXT");
    let head = match &prev {
        Some(c) => format!("After {}", c.name(cx.flats)),
        None => "Good chords to start with".to_string(),
    };
    label(ui, pos2(r.right() - 20.0, t + 26.0), Align2::RIGHT_CENTER, head, 13.0, TEXT);

    let rows: Vec<Suggestion> = suggest_next(prev.as_ref(), cx.key, cx.scale, cx.genre).into_iter().take(if cx.project.melody.is_empty() { 4 } else { 3 }).collect();
    for (i, s) in rows.iter().enumerate() {
        let y = t + 50.0 + i as f32 * 58.0;
        let rn = roman(cx.key, cx.scale, &s.chord);
        let (resp, plus) = tile(ui, Rect::from_min_size(pos2(l, y), vec2(88.0, 50.0)), Id::new(("idea", i)), &s.chord, cx.flats, &rn, s.chord.in_key(cx.key, cx.scale), false);
        let resp = chord_tip(resp, &s.chord, cx.key, cx.scale, cx.flats);
        if st.chip(cx, &resp, s.chord) || plus {
            cx.add_chord(st, s.chord);
        }
        wrapped(ui, Rect::from_min_size(pos2(l + 100.0, y + 3.0), vec2(r.width() - 140.0, 48.0)), &short_reason(&s.reason), 11.5, DIM);
    }
    if !cx.project.melody.is_empty() {
        if big_button(ui, Rect::from_min_size(pos2(l, r.bottom() - 50.0), vec2(r.width() - 40.0, 34.0)), "fit", "Fit the chords to my melody", Btn::Normal,
            "Replace each chord with the best match for the melody notes over it (lengths stay).") {
            reharmonise(cx);
        }
    }
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

// ---------------------------------------------------------------------------------------------
// Drag and drop
// ---------------------------------------------------------------------------------------------

/// Completes a chord drag by dropping it on the progression where the pointer is.
pub fn finish_drag(ui: &mut Ui, cx: &Cx, st: &mut State) {
    let Some(chord) = st.drag else { return };
    let released = ui.input(|i| i.pointer.any_released() || !i.pointer.any_down());
    if !released {
        return;
    }
    st.drag = None;
    let o = ui.max_rect().min.to_vec2();
    let area = Rect::from_min_size(pos2(MARGIN, 88.0) + o, vec2(W - 2.0 * MARGIN, 624.0));
    let g = geo(area, cx.project.beats());
    let Some(pos) = ui.input(|i| i.pointer.latest_pos()) else { return };
    if !g.card.contains(pos) {
        return;
    }
    let beat = ((pos.x - g.lane.left()) / g.ppb).max(0.0);
    let beats = st.sel.and_then(|s| cx.project.item(s)).map(|i| i.beats).or_else(|| cx.project.placed().last().map(|p| p.item_beats)).unwrap_or(4.0);
    let new = cx.p.project.edit(|pr| if pr.placed().is_empty() { pr.push_chord(chord, beats) } else { pr.insert_chord_at(beat, chord, beats) });
    st.sel = Some(new);
    st.cur_section = new.section;
}

impl State {
    /// Press = listen while held; dragging the chip starts moving the chord; returns true on double-click.
    pub fn chip(&mut self, cx: &Cx, resp: &Response, chord: Chord) -> bool {
        let down = resp.is_pointer_button_down_on();
        match &self.audition {
            None if down => cx.start_audition(self, resp.id, &chord),
            Some((id, _)) if !down && *id == resp.id => cx.stop_audition(self),
            _ => {}
        }
        if resp.drag_started() {
            self.drag = Some(chord);
        }
        resp.double_clicked()
    }
}
