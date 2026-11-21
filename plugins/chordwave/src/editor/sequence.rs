//! The sequence: chords as blocks on a timeline, grouped in sections.

use super::*;

const SECTION_COLORS: [Color32; 4] = [Color32::from_rgb(84, 226, 206), Color32::from_rgb(255, 84, 160), Color32::from_rgb(255, 176, 64), Color32::from_rgb(182, 128, 255)];

/// Where the blocks are drawn for a given panel rectangle and sketch length.
pub struct Geo {
    pub x0: f32,
    pub ppb: f32,
    pub ruler_y: f32,
    pub strip_y: f32,
    pub blocks: Rect,
    pub inspector: Rect,
}

pub fn geo(rect: Rect, beats: f32) -> Geo {
    let inner = Rect::from_min_max(pos2(rect.left() + 10.0, rect.top() + TITLE_H), pos2(rect.right() - 10.0, rect.bottom() - 6.0));
    // Leave room after the last chord for dropping a new one.
    let span = (beats + 4.0).max(16.0);
    let ppb = (inner.width() / span).clamp(4.0, 70.0);
    let blocks = Rect::from_min_size(pos2(inner.left(), inner.top() + 26.0), vec2(inner.width(), 40.0));
    Geo {
        x0: inner.left(),
        ppb,
        ruler_y: inner.top(),
        strip_y: inner.top() + 12.0,
        blocks,
        inspector: Rect::from_min_size(pos2(inner.left(), inner.top() + 70.0), vec2(inner.width(), 26.0)),
    }
}

fn snap_beats(x: f32) -> f32 {
    (x * 2.0).round() / 2.0
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let g = geo(rect, cx.project.beats());
    let bars = (cx.project.beats() / BEATS_PER_BAR).ceil() as i32;
    let title = format!("SEQUENCE  ·  {} bar{}  ·  {} chord{}", bars, if bars == 1 { "" } else { "s" }, cx.project.placed().len(), if cx.project.placed().len() == 1 { "" } else { "s" });
    panel(ui, rect, &title, ACCENT);
    toolbar(ui, cx, st, rect);

    // Timeline background and ruler.
    let end_x = (g.x0 + (cx.project.beats() + 4.0).max(16.0) * g.ppb).min(g.blocks.right());
    let lane = Rect::from_min_max(pos2(g.x0, g.blocks.top()), pos2(end_x, g.blocks.bottom()));
    ui.painter().rect_filled(lane, 4.0, INSET);
    let total_bars = ((end_x - g.x0) / (g.ppb * BEATS_PER_BAR)).floor() as i32 + 1;
    for b in 0..=total_bars {
        let x = g.x0 + b as f32 * BEATS_PER_BAR * g.ppb;
        if x > end_x + 1.0 {
            break;
        }
        ui.painter().line_segment([pos2(x, g.ruler_y + 8.0), pos2(x, g.blocks.bottom())], st_line(b % 4 == 0));
        ui.painter().text(pos2(x + 3.0, g.ruler_y + 5.0), Align2::LEFT_CENTER, format!("{}", b + 1), FontId::proportional(8.5), FAINT);
    }

    let placed = cx.project.placed();
    let playhead = cx.shared.playhead.load(Ordering::Relaxed);
    let mut reorder: Option<(Sel, i32)> = None;

    // Section strips.
    let mut start = 0.0;
    for (si, s) in cx.project.sections.iter().enumerate() {
        let beats = s.beats();
        let color = SECTION_COLORS[si % SECTION_COLORS.len()];
        let x = g.x0 + start * g.ppb;
        let w = (beats * g.ppb - 2.0).max(16.0);
        let strip = Rect::from_min_size(pos2(x, g.strip_y), vec2(w, 13.0));
        let resp = ui.interact(strip, Id::new(("section", si)), Sense::click());
        let active = si == st.cur_section;
        ui.painter().rect_filled(strip, 3.0, color.gamma_multiply(if active { 0.4 } else { 0.18 }));
        ui.painter_at(strip).text(pos2(strip.left() + 5.0, strip.center().y), Align2::LEFT_CENTER, &s.name, FontId::proportional(9.0), if active { Color32::WHITE } else { color });
        if resp.clicked() {
            st.cur_section = si;
            st.sel = None;
        }
        resp.on_hover_text("Section. Click to make it the target for new chords and for the section buttons.");
        start += beats;
    }
    if cx.project.sections.get(st.cur_section).map_or(false, |s| s.items.is_empty()) {
        let x = g.x0 + start * g.ppb;
        ui.painter().text(pos2(x + 8.0, g.strip_y + 6.0), Align2::LEFT_CENTER, "new section: add chords", FontId::proportional(9.0), FAINT);
    }

    // Chord blocks.
    for (idx, p) in placed.iter().enumerate() {
        let x = g.x0 + p.start * g.ppb;
        let w = (p.item_beats * g.ppb - 2.0).max(6.0);
        let block = Rect::from_min_size(pos2(x, g.blocks.top() + 2.0), vec2(w, g.blocks.height() - 4.0));
        let handle = Rect::from_min_max(pos2(block.right() - 6.0, block.top()), block.max);
        let body = Rect::from_min_max(block.min, pos2(block.right() - 6.0, block.bottom()));
        let selected = st.sel == Some(p.sel);
        let col = kind_color(p.chord.kind());
        let now = playhead >= 0.0 && playhead >= p.start && playhead < p.start + p.item_beats;

        let resp = ui.interact(body, Id::new(("blk", idx)), Sense::click_and_drag());
        let hresp = ui.interact(handle, Id::new(("blk-handle", idx)), Sense::drag());
        let painter = ui.painter();
        painter.rect_filled(block, 5.0, col.gamma_multiply(if selected { 0.42 } else if resp.hovered() { 0.3 } else { 0.2 }));
        painter.rect_stroke(block, 5.0, st_stroke(if selected { 1.8 } else { 1.0 }, if selected { ACCENT } else if now { AQUA } else { col.gamma_multiply(0.55) }), StrokeKind::Inside);
        painter.rect_filled(Rect::from_min_max(pos2(block.right() - 4.0, block.top() + 6.0), pos2(block.right() - 2.5, block.bottom() - 6.0)), 1.0, if hresp.hovered() || hresp.dragged() { ACCENT } else { col.gamma_multiply(0.5) });
        let clip = ui.painter_at(body);
        clip.text(pos2(block.left() + 8.0, block.center().y - 6.0), Align2::LEFT_CENTER, p.chord.name(cx.flats), FontId::proportional(14.0), TEXT);
        clip.text(pos2(block.left() + 8.0, block.center().y + 8.0), Align2::LEFT_CENTER, roman(cx.key, cx.scale, &p.chord), FontId::proportional(9.5), DIM);
        if !p.chord.in_key(cx.key, cx.scale) {
            painter.circle_stroke(pos2(block.right() - 14.0, block.top() + 8.0), 3.0, st_stroke(1.2, col));
        }

        // Select and listen while pressed.
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
        // Drag sideways to reorder.
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let left = idx.checked_sub(1).and_then(|i| placed.get(i)).filter(|n| n.sel.section == p.sel.section);
                let right = placed.get(idx + 1).filter(|n| n.sel.section == p.sel.section);
                if let Some(n) = left {
                    if pos.x < g.x0 + (n.start + n.item_beats * 0.5) * g.ppb {
                        reorder = Some((p.sel, -1));
                    }
                }
                if let Some(n) = right {
                    if pos.x > g.x0 + (n.start + n.item_beats * 0.5) * g.ppb {
                        reorder = Some((p.sel, 1));
                    }
                }
            }
        }
        // Drag the right edge to change the length.
        if hresp.dragged() {
            if let Some(pos) = hresp.interact_pointer_pos() {
                let fine = ui.input(|i| i.modifiers.shift);
                let beats = (pos.x - block.left()) / g.ppb;
                let beats = if fine { (beats * 2.0).round() / 2.0 } else { beats.round() };
                let sel = p.sel;
                cx.p.project.edit(|pr| pr.set_beats(sel, beats.max(0.5)));
            }
        }
        if hresp.hovered() || hresp.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
        let _ = resp.on_hover_text("Click: select and listen  ·  Drag: reorder  ·  Drag the right edge: length (Shift = half beats)");
    }
    if let Some((sel, dir)) = reorder {
        let new = cx.p.project.edit(|pr| pr.move_item(sel, dir));
        st.sel = Some(new);
    }

    // Playhead.
    if playhead >= 0.0 && playhead < cx.project.beats() {
        let x = g.x0 + playhead * g.ppb;
        ui.painter().line_segment([pos2(x, g.strip_y), pos2(x, g.blocks.bottom())], st_stroke(1.5, Color32::WHITE));
    }

    // Insertion marker while a chord is being dragged in.
    if let Some(chord) = st.drag {
        if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
            if lane.expand2(vec2(0.0, 12.0)).contains(pos) {
                let beat = ((pos.x - g.x0) / g.ppb).max(0.0);
                let boundary = boundary_near(&placed, beat);
                let x = g.x0 + boundary * g.ppb;
                ui.painter().line_segment([pos2(x, g.blocks.top() - 2.0), pos2(x, g.blocks.bottom() + 2.0)], st_stroke(2.5, ACCENT));
            }
        }
        ghost(ui, &chord, cx.flats);
    }

    if placed.is_empty() {
        ui.painter().text(lane.center(), Align2::CENTER_CENTER, "Empty: double-click a chord in the palette, or drag one here, or pick a progression", FontId::proportional(11.0), FAINT);
    }

    inspector(ui, cx, st, &g);

    // Keyboard shortcuts for the selected chord.
    if let Some(sel) = st.sel {
        let delete = ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if delete && ui.ctx().memory(|m| m.focused().is_none()) {
            cx.p.project.edit(|pr| pr.remove(sel));
            st.sel = None;
        }
    }
}

fn st_line(strong: bool) -> egui::Stroke {
    st_stroke(1.0, if strong { LINE } else { Color32::from_rgb(24, 27, 33) })
}

fn st_stroke(w: f32, c: Color32) -> egui::Stroke {
    plugin_common::ui::st(w, c)
}

/// The start (in beats) of the chord boundary nearest to `beat`, including the end of the sketch.
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

fn toolbar(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let y = rect.top() + 4.0;
    let mut x = rect.right() - 12.0;
    let mut place = |w: f32| {
        x -= w;
        let r = Rect::from_min_size(pos2(x, y), vec2(w, 19.0));
        x -= 6.0;
        r
    };
    if button(ui, place(54.0), "seq-clear", "CLEAR", DIM, "Remove every chord (the melody stays).") {
        cx.p.project.edit(|pr| pr.sections = vec![Section { name: "A".into(), items: Vec::new() }]);
        st.sel = None;
        st.cur_section = 0;
    }
    if button(ui, place(92.0), "seq-copy", "COPY NAMES", DIM, "Copy the chord names to the clipboard, one line per section.") {
        let flats = cx.flats;
        let text: Vec<String> = cx.project.sections.iter().map(|s| format!("{}: {}", s.name, s.items.iter().map(|i| i.chord.name(flats)).collect::<Vec<_>>().join(" | "))).collect();
        ui.ctx().copy_text(text.join("\n"));
        cx.flash(st, "Chord names copied");
    }
    if button(ui, place(70.0), "seq-delsec", "DEL SECTION", DIM, "Delete the selected section with its chords and the melody over it.") {
        let si = st.cur_section;
        cx.p.project.edit(|pr| pr.remove_section(si));
        st.sel = None;
    }
    if button(ui, place(70.0), "seq-dupsec", "DUP SECTION", DIM, "Copy the selected section and paste it right after itself: ready for the next verse or chorus.") {
        let si = st.cur_section;
        cx.p.project.edit(|pr| pr.duplicate_section(si));
        st.cur_section = si + 1;
        st.sel = None;
    }
    if button(ui, place(74.0), "seq-addsec", "+ SECTION", AQUA, "Start a new, empty section after the last one. Chords you add go into the selected section.") {
        let new = cx.p.project.edit(|pr| pr.add_section());
        st.cur_section = new;
        st.sel = None;
    }
}

fn inspector(ui: &mut Ui, cx: &Cx, st: &mut State, g: &Geo) {
    let r = g.inspector;
    let Some(sel) = st.sel else {
        ui.painter().text(pos2(r.left() + 4.0, r.center().y), Align2::LEFT_CENTER,
            "Select a chord to see what it does harmonically and to change its length, voicing or bass note.", FontId::proportional(10.5), FAINT);
        return;
    };
    let Some(item) = cx.project.item(sel).cloned() else { return };
    let chord = item.chord;
    let f = function_of(cx.key, cx.scale, &chord);
    let notes: Vec<&str> = chord.pitch_classes().iter().map(|p| pc_name(*p, cx.flats)).collect();
    let painter = ui.painter();
    painter.text(pos2(r.left() + 4.0, r.top() + 7.0), Align2::LEFT_CENTER, format!("{}   {}   ·   {}", chord.name(cx.flats), roman(cx.key, cx.scale, &chord), notes.join(" ")), FontId::proportional(12.0), TEXT);
    let line = format!("{}: {}", f.label(), f.explain());
    painter.text(pos2(r.left() + 4.0, r.top() + 20.0), Align2::LEFT_CENTER, ellipsize(&line, 84), FontId::proportional(9.5), function_color(f));

    let y = r.top() + 1.0;
    let mut x = r.left() + 430.0;
    let mut slot = |w: f32| {
        let rc = Rect::from_min_size(pos2(x, y), vec2(w, 22.0));
        x += w + 5.0;
        rc
    };
    if button(ui, slot(24.0), "ch-left", "‹", TEXT, "Move this chord one place earlier") {
        let new = cx.p.project.edit(|pr| pr.move_item(sel, -1));
        st.sel = Some(new);
    }
    if button(ui, slot(24.0), "ch-right", "›", TEXT, "Move this chord one place later") {
        let new = cx.p.project.edit(|pr| pr.move_item(sel, 1));
        st.sel = Some(new);
    }
    let len_rect = slot(26.0);
    if button(ui, len_rect, "ch-shorter", "−", TEXT, "Shorter by one beat") {
        cx.p.project.edit(|pr| pr.set_beats(sel, (item.beats - 1.0).max(0.5)));
    }
    let lbl = slot(62.0);
    ui.painter().text(lbl.center(), Align2::CENTER_CENTER, format!("{} beats", item.beats), FontId::proportional(10.5), DIM);
    if button(ui, slot(26.0), "ch-longer", "+", TEXT, "Longer by one beat") {
        cx.p.project.edit(|pr| pr.set_beats(sel, item.beats + 1.0));
    }
    let inv_names = ["Auto voicing", "Root position", "1st inversion", "2nd inversion", "3rd inversion"];
    let inv_label = inv_names[item.inversion.map_or(0, |i| (i as usize + 1).min(4))];
    if let Some(i) = dd(ui, slot(120.0), "ch-inv", inv_label, &inv_names, DIM,
        "Auto picks the inversion that moves the least from the previous chord (smooth voice leading). Or fix one yourself.") {
        cx.p.project.edit(|pr| {
            if let Some(it) = pr.item_mut(sel) {
                it.inversion = if i == 0 { None } else { Some(i as u8 - 1) };
            }
        });
    }
    let mut slash: Vec<String> = vec!["No slash bass".into()];
    slash.extend((0..12u8).map(|p| format!("/{}", pc_name(p, cx.flats))));
    let slash_label = chord.bass.map_or("No slash bass".to_string(), |b| format!("/{}", pc_name(b, cx.flats)));
    if let Some(i) = dd(ui, slot(104.0), "ch-slash", &slash_label, &slash, DIM, "A slash chord puts another note in the bass, e.g. C/E. Great for stepwise bass lines.") {
        cx.p.project.edit(|pr| {
            if let Some(it) = pr.item_mut(sel) {
                it.chord = it.chord.with_bass(if i == 0 { None } else { Some(i as u8 - 1) });
            }
        });
    }
    if button(ui, slot(64.0), "ch-dup", "DUPLICATE", DIM, "Copy this chord right after itself") {
        let new = cx.p.project.edit(|pr| pr.duplicate_item(sel));
        st.sel = Some(new);
    }
    if button(ui, slot(52.0), "ch-del", "DELETE", ACCENT, "Remove this chord (Delete key)") {
        cx.p.project.edit(|pr| pr.remove(sel));
        st.sel = None;
    }
}

/// Called after every panel is drawn: completes a chord drag by dropping it where the pointer is.
pub fn finish_drag(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let Some(chord) = st.drag else { return };
    let released = ui.input(|i| i.pointer.any_released() || !i.pointer.any_down());
    if !released {
        return;
    }
    st.drag = None;
    let g = geo(rect, cx.project.beats());
    let Some(pos) = ui.input(|i| i.pointer.latest_pos()) else { return };
    if !Rect::from_min_max(pos2(g.x0, g.strip_y - 4.0), pos2(rect.right(), g.blocks.bottom() + 12.0)).contains(pos) {
        return;
    }
    let beat = ((pos.x - g.x0) / g.ppb).max(0.0);
    let beats = st.sel.and_then(|s| cx.project.item(s)).map(|i| i.beats).or_else(|| cx.project.placed().last().map(|p| p.item_beats)).unwrap_or(4.0);
    let new = cx.p.project.edit(|pr| {
        if pr.placed().is_empty() {
            pr.push_chord(chord, beats)
        } else {
            pr.insert_chord_at(beat, chord, beats)
        }
    });
    st.sel = Some(new);
    st.cur_section = new.section;
    let _ = snap_beats;
}
