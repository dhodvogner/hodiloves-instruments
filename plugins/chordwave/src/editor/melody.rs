//! Step 3: the melody. A large piano roll with obvious bars and beats, labelled rows, the chords
//! drawn above it, and (by default) only the notes of your key, so every note you place fits.

use super::*;
use crate::generate::*;

#[derive(Clone, Copy)]
enum Drag {
    Create { idx: usize, start: f32 },
    Move { idx: usize, grab: f32 },
    Resize { idx: usize },
}

#[derive(Default)]
pub struct RollState {
    drag: Option<Drag>,
    sel: Option<usize>,
    sounding: Option<u8>,
    key_down: Option<u8>,
}

const LABEL_W: f32 = 62.0;
const BAND_H: f32 = 30.0;
const RULER_H: f32 = 30.0;
const OUTSIDE: Color32 = Color32::from_rgb(255, 120, 90);

struct View {
    grid: Rect,
    rows: Vec<i32>,
    row_h: f32,
    ppb: f32,
    sx: f32,
    sy: f32,
}

impl View {
    fn x(&self, beat: f32) -> f32 {
        self.grid.left() + beat * self.ppb - self.sx
    }
    fn beat(&self, x: f32) -> f32 {
        (x - self.grid.left() + self.sx) / self.ppb
    }
    fn row_y(&self, idx: usize) -> f32 {
        self.grid.top() + idx as f32 * self.row_h - self.sy
    }
    fn row_at(&self, y: f32) -> usize {
        (((y - self.grid.top() + self.sy) / self.row_h).floor().max(0.0) as usize).min(self.rows.len().saturating_sub(1))
    }
    fn note_rect(&self, n: &Note) -> Option<Rect> {
        let idx = self.rows.iter().position(|p| *p == n.pitch as i32)?;
        Some(Rect::from_min_size(pos2(self.x(n.start), self.row_y(idx) + 1.5), vec2((n.len * self.ppb - 2.0).max(6.0), self.row_h - 3.0)))
    }
}

fn snap_value(st: &State) -> f32 {
    SNAPS[st.snap.min(SNAPS.len() - 1)].1
}

fn build_rows(cx: &Cx, st: &State) -> Vec<i32> {
    let (mlo, mhi) = cx.p.melody_range();
    let (mut lo, mut hi) = (mlo - 2, mhi + 2);
    for n in &cx.project.melody {
        lo = lo.min(n.pitch as i32 - 1);
        hi = hi.max(n.pitch as i32 + 1);
    }
    let mask = scale_mask(cx.key, cx.scale);
    (lo.max(0)..=hi.min(127))
        .rev()
        .filter(|p| st.show_all || mask[p.rem_euclid(12) as usize] || cx.project.melody.iter().any(|n| n.pitch as i32 == *p))
        .collect()
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    label(ui, pos2(area.left(), area.top() + 14.0), Align2::LEFT_CENTER, "Write a melody", 26.0, TEXT);
    label(ui, pos2(area.left(), area.top() + 46.0), Align2::LEFT_CENTER,
        "Click in the grid to add a note. Drag a note to move it, drag its right edge to resize it, right-click to delete it.", 13.5, DIM);

    toolbar(ui, cx, st, Rect::from_min_size(pos2(area.left(), area.top() + 70.0), vec2(area.width(), 40.0)));

    let roll = Rect::from_min_max(pos2(area.left(), area.top() + 122.0), pos2(area.right(), area.bottom() - 30.0));
    ui.painter().rect_filled(roll, 12.0, PANEL);
    let grid = Rect::from_min_max(pos2(roll.left() + LABEL_W, roll.top() + BAND_H + RULER_H), roll.max - vec2(8.0, 8.0));

    // Rows and zoom.
    let rows = build_rows(cx, st);
    let total_beats = cx.project.beats().max(BEATS_PER_BAR * 2.0);
    let row_h = (grid.height() / rows.len().max(1) as f32).clamp(22.0, 40.0);
    let fit = grid.width() / total_beats;
    let ppb = st.zoom.unwrap_or_else(|| fit.clamp(24.0, 110.0)).clamp(14.0, 220.0);
    let content_w = total_beats * ppb;
    let content_h = rows.len() as f32 * row_h;
    if !st.roll_y_init {
        let (mlo, mhi) = cx.p.melody_range();
        let target = if cx.project.melody.is_empty() { (mlo + mhi) / 2 } else { cx.project.melody.iter().map(|n| n.pitch as i32).sum::<i32>() / cx.project.melody.len() as i32 };
        let idx = rows.iter().position(|p| *p <= target).unwrap_or(0);
        st.roll_y = (idx as f32 * row_h - grid.height() / 2.0).max(0.0);
        st.roll_y_init = true;
    }
    if ui.rect_contains_pointer(grid) {
        let (d, shift) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.shift));
        if shift {
            st.roll_x -= d.y + d.x;
        } else {
            st.roll_y -= d.y;
            st.roll_x -= d.x;
        }
    }
    st.roll_x = st.roll_x.clamp(0.0, (content_w - grid.width()).max(0.0));
    st.roll_y = st.roll_y.clamp(0.0, (content_h - grid.height()).max(0.0));
    let view = View { grid, rows, row_h, ppb, sx: st.roll_x, sy: st.roll_y };

    let mask = scale_mask(cx.key, cx.scale);
    let placed = cx.project.placed();
    let g = ui.painter_at(grid);

    // Rows.
    for (idx, pitch) in view.rows.iter().enumerate() {
        let pc = pitch.rem_euclid(12) as u8;
        let y = view.row_y(idx);
        let col = if !mask[pc as usize] {
            Color32::from_rgb(16, 17, 22)
        } else if pc == cx.key {
            Color32::from_rgb(48, 38, 56)
        } else {
            Color32::from_rgb(31, 35, 44)
        };
        g.rect_filled(Rect::from_min_size(pos2(grid.left(), y), vec2(grid.width(), row_h)), 0.0, col);
        g.line_segment([pos2(grid.left(), y + row_h), pos2(grid.right(), y + row_h)], st_s(1.0, Color32::from_rgb(20, 22, 28)));
    }
    // Bars (alternate shading), beats and subdivisions.
    let snap = snap_value(st);
    let first = (view.sx / ppb).floor() as i32 - 1;
    let last = ((view.sx + grid.width()) / ppb).ceil() as i32 + 1;
    for bar in (first.div_euclid(4))..=(last.div_euclid(4)) {
        if bar < 0 || bar % 2 == 1 {
            continue;
        }
        let r = Rect::from_min_max(pos2(view.x(bar as f32 * 4.0), grid.top()), pos2(view.x(bar as f32 * 4.0 + 4.0), grid.bottom()));
        g.rect_filled(r, 0.0, Color32::from_white_alpha(6));
    }
    // Chord tones glow under each chord.
    for p in &placed {
        let col = kind_color(p.chord.kind());
        let (x0, x1) = (view.x(p.start), view.x(p.start + p.item_beats));
        let pcs = p.chord.pitch_classes();
        for (idx, pitch) in view.rows.iter().enumerate() {
            if pcs.contains(&(pitch.rem_euclid(12) as u8)) {
                g.rect_filled(Rect::from_min_max(pos2(x0, view.row_y(idx)), pos2(x1 - 1.0, view.row_y(idx) + row_h)), 0.0, col.gamma_multiply(0.17));
            }
        }
    }
    let mut b = first;
    while b <= last {
        let x = view.x(b as f32);
        let is_bar = b.rem_euclid(4) == 0;
        if b >= 0 {
            g.line_segment([pos2(x, grid.top()), pos2(x, grid.bottom())], st_s(if is_bar { 2.0 } else { 1.0 }, if is_bar { Color32::from_rgb(120, 128, 148) } else { Color32::from_rgb(56, 62, 76) }));
            if ppb * snap >= 12.0 {
                let mut s = snap;
                while s < 1.0 - 0.001 {
                    let sx = view.x(b as f32 + s);
                    g.line_segment([pos2(sx, grid.top()), pos2(sx, grid.bottom())], st_s(1.0, Color32::from_rgb(36, 40, 50)));
                    s += snap;
                }
            }
        }
        b += 1;
    }

    // Chord band and ruler.
    let band = Rect::from_min_size(pos2(grid.left(), roll.top() + 6.0), vec2(grid.width(), BAND_H - 8.0));
    let bp = ui.painter_at(Rect::from_min_max(pos2(grid.left(), roll.top()), pos2(grid.right(), grid.top())));
    for p in &placed {
        let col = kind_color(p.chord.kind());
        let (x0, x1) = (view.x(p.start), view.x(p.start + p.item_beats));
        if x1 < band.left() || x0 > band.right() {
            continue;
        }
        let r = Rect::from_min_max(pos2(x0 + 1.0, band.top()), pos2(x1 - 2.0, band.bottom()));
        bp.rect_filled(r, 6.0, col.gamma_multiply(0.35));
        bp.text(pos2((r.left() + 8.0).max(band.left() + 6.0), r.center().y), Align2::LEFT_CENTER, format!("{}   {}", p.chord.name(cx.flats), roman(cx.key, cx.scale, &p.chord)), FontId::proportional(13.0), TEXT);
    }
    let ry = grid.top() - RULER_H;
    let mut b = first.max(0);
    while b <= last {
        let x = view.x(b as f32);
        if x >= grid.left() - 1.0 && x <= grid.right() {
            if b % 4 == 0 {
                bp.line_segment([pos2(x, ry + 4.0), pos2(x, grid.top())], st_s(2.0, Color32::from_rgb(120, 128, 148)));
                bp.text(pos2(x + 6.0, ry + 14.0), Align2::LEFT_CENTER, format!("{}", b / 4 + 1), FontId::proportional(15.0), TEXT);
            } else {
                bp.line_segment([pos2(x, ry + 16.0), pos2(x, grid.top())], st_s(1.0, Color32::from_rgb(80, 86, 100)));
                if ppb >= 34.0 {
                    bp.text(pos2(x + 4.0, ry + 20.0), Align2::LEFT_CENTER, format!("{}", b % 4 + 1), FontId::proportional(10.5), FAINT);
                }
            }
        }
        b += 1;
    }
    ui.painter().text(pos2(roll.left() + 12.0, ry + 14.0), Align2::LEFT_CENTER, "Bar", FontId::proportional(11.0), FAINT);

    // Row labels (press to hear the note).
    key_labels(ui, cx, st, &view, &mask, roll);

    // Notes.
    for (i, n) in cx.project.melody.iter().enumerate() {
        let Some(r) = view.note_rect(n) else { continue };
        if r.right() < grid.left() || r.left() > grid.right() {
            continue;
        }
        let pc = n.pitch % 12;
        let chord = cx.project.chord_at(n.start + 0.001).map(|p| p.chord);
        let col = if !mask[pc as usize] {
            OUTSIDE
        } else if chord.is_some_and(|c| c.contains_pc(pc)) {
            AQUA
        } else {
            MINOR_C
        };
        let selected = st.roll.sel == Some(i);
        g.rect_filled(r, 5.0, col.gamma_multiply(0.55 + 0.35 * (n.vel as f32 / 127.0)));
        g.rect_stroke(r, 5.0, st_s(if selected { 2.5 } else { 1.2 }, if selected { Color32::WHITE } else { col }), StrokeKind::Inside);
        if r.width() > 34.0 {
            g.text(pos2(r.left() + 7.0, r.center().y), Align2::LEFT_CENTER, midi_name(n.pitch), FontId::proportional(11.0), Color32::from_black_alpha(220));
        }
    }

    // Playhead.
    let head = cx.shared.playhead.load(Ordering::Relaxed);
    if head >= 0.0 {
        let x = view.x(head);
        g.line_segment([pos2(x, grid.top()), pos2(x, grid.bottom())], st_s(2.0, Color32::WHITE));
    }

    interact(ui, cx, st, &view, total_beats, snap);

    // Legend.
    let ly = area.bottom() - 12.0;
    let mut lx = area.left() + 4.0;
    for (c, text) in [(AQUA, "fits the chord"), (MINOR_C, "passing note"), (OUTSIDE, "outside the key")] {
        ui.painter().rect_filled(Rect::from_center_size(pos2(lx + 6.0, ly), vec2(12.0, 9.0)), 3.0, c);
        label(ui, pos2(lx + 18.0, ly), Align2::LEFT_CENTER, text, 12.0, DIM);
        lx += 38.0 + text.len() as f32 * 6.6;
    }
    label(ui, pos2(area.right(), ly), Align2::RIGHT_CENTER, "Mouse wheel scrolls up and down, Shift + wheel scrolls sideways", 11.5, FAINT);
}

fn st_s(w: f32, c: Color32) -> egui::Stroke {
    plugin_common::ui::st(w, c)
}

fn key_labels(ui: &mut Ui, cx: &Cx, st: &mut State, view: &View, mask: &[bool; 12], roll: Rect) {
    let strip = Rect::from_min_max(pos2(roll.left() + 8.0, view.grid.top()), pos2(view.grid.left() - 4.0, view.grid.bottom()));
    let p = ui.painter_at(strip);
    for (idx, pitch) in view.rows.iter().enumerate() {
        let pc = pitch.rem_euclid(12) as u8;
        let y = view.row_y(idx);
        let r = Rect::from_min_size(pos2(strip.left(), y + 1.0), vec2(strip.width(), view.row_h - 2.0));
        let root = pc == cx.key;
        p.rect_filled(r, 5.0, if root { ACCENT.gamma_multiply(0.35) } else if mask[pc as usize] { RAISED } else { INSET });
        p.text(r.center(), Align2::CENTER_CENTER, midi_name(*pitch as u8), FontId::proportional(12.5), if root { TEXT } else { Color32::from_rgb(190, 196, 208) });
    }
    let resp = ui.interact(strip, Id::new("roll-keys"), Sense::click_and_drag());
    let down = resp.is_pointer_button_down_on();
    let hit = if down { resp.interact_pointer_pos().map(|p| view.rows[view.row_at(p.y)] as u8) } else { None };
    if hit != st.roll.key_down {
        if let Some(old) = st.roll.key_down {
            cx.note_off(old);
        }
        if let Some(new) = hit {
            cx.note_on(new);
        }
        st.roll.key_down = hit;
    }
}

fn interact(ui: &mut Ui, cx: &Cx, st: &mut State, view: &View, total_beats: f32, snap: f32) {
    let resp = ui.interact(view.grid, Id::new("roll-grid"), Sense::click_and_drag());
    let pos = ui.input(|i| i.pointer.interact_pos());
    let pressed = ui.input(|i| i.pointer.primary_pressed());
    let down = ui.input(|i| i.pointer.primary_down());
    if view.rows.is_empty() {
        return;
    }
    let hit_note = |p: egui::Pos2| cx.project.melody.iter().enumerate().rev().find(|(_, n)| view.note_rect(n).is_some_and(|r| r.expand2(vec2(1.0, 0.0)).contains(p))).map(|(i, _)| i);

    // A faint preview of the note a click would add.
    if resp.hovered() && st.roll.drag.is_none() {
        if let Some(p) = pos {
            if hit_note(p).is_none() {
                let beat = ((view.beat(p.x) / snap).floor() * snap).max(0.0);
                let idx = view.row_at(p.y);
                let r = Rect::from_min_size(pos2(view.x(beat), view.row_y(idx) + 1.5), vec2((snap * view.ppb - 2.0).max(6.0), view.row_h - 3.0));
                ui.painter_at(view.grid).rect_filled(r, 5.0, Color32::from_white_alpha(34));
            }
        }
    }

    if resp.secondary_clicked() {
        if let Some(i) = pos.and_then(hit_note) {
            st.undo_melody = Some(cx.project.melody.clone());
            cx.p.project.edit(|pr| {
                if i < pr.melody.len() {
                    pr.melody.remove(i);
                }
            });
            st.roll.sel = None;
        }
    }

    if pressed && resp.hovered() {
        if let Some(p) = pos {
            let beat = view.beat(p.x);
            let pitch = view.rows[view.row_at(p.y)];
            match hit_note(p) {
                Some(i) => {
                    let n = cx.project.melody[i];
                    st.roll.sel = Some(i);
                    let edge = view.note_rect(&n).map_or(0.0, |r| r.right());
                    if p.x > edge - 9.0 && n.len * view.ppb > 22.0 {
                        st.roll.drag = Some(Drag::Resize { idx: i });
                    } else {
                        st.roll.drag = Some(Drag::Move { idx: i, grab: beat - n.start });
                    }
                    cx.note_on(n.pitch);
                    st.roll.sounding = Some(n.pitch);
                }
                None if beat >= 0.0 && beat < total_beats => {
                    let start = ((beat / snap).floor() * snap).max(0.0);
                    st.undo_melody = Some(cx.project.melody.clone());
                    let idx = cx.p.project.edit(|pr| {
                        pr.melody.push(Note::new(start, snap.min(total_beats - start).max(0.0625), pitch as u8, 100));
                        pr.melody.len() - 1
                    });
                    st.roll.sel = Some(idx);
                    st.roll.drag = Some(Drag::Create { idx, start });
                    cx.note_on(pitch as u8);
                    st.roll.sounding = Some(pitch as u8);
                }
                None => {}
            }
        }
    }

    if let (Some(drag), true, Some(p)) = (st.roll.drag, down, pos) {
        let beat = view.beat(p.x);
        let pitch = view.rows[view.row_at(p.y)] as u8;
        let mut new_pitch = None;
        match drag {
            Drag::Create { idx, start } => {
                let end = ((beat / snap).round() * snap).min(total_beats);
                let len = (end - start).max(snap);
                cx.p.project.edit(|pr| {
                    if let Some(n) = pr.melody.get_mut(idx) {
                        n.len = len.min(total_beats - n.start).max(0.0625);
                    }
                });
            }
            Drag::Move { idx, grab } => {
                let start = (((beat - grab) / snap).round() * snap).clamp(0.0, (total_beats - snap).max(0.0));
                cx.p.project.edit(|pr| {
                    if let Some(n) = pr.melody.get_mut(idx) {
                        n.start = start;
                        if n.pitch != pitch {
                            n.pitch = pitch;
                            new_pitch = Some(pitch);
                        }
                        n.len = n.len.min(total_beats - n.start).max(0.0625);
                    }
                });
            }
            Drag::Resize { idx } => {
                let end = ((beat / snap).round() * snap).min(total_beats);
                cx.p.project.edit(|pr| {
                    if let Some(n) = pr.melody.get_mut(idx) {
                        n.len = (end - n.start).max(snap.min(0.25)).min(total_beats - n.start);
                    }
                });
            }
        }
        if let Some(np) = new_pitch {
            if let Some(old) = st.roll.sounding.replace(np) {
                cx.note_off(old);
            }
            cx.note_on(np);
        }
    }
    if st.roll.drag.is_some() && !down {
        st.roll.drag = None;
        if let Some(old) = st.roll.sounding.take() {
            cx.note_off(old);
        }
        cx.p.project.edit(|pr| pr.melody.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch))));
        st.roll.sel = None;
    }
    if resp.hovered() || st.roll.drag.is_some() {
        if let Some(p) = pos {
            if hit_note(p).is_some_and(|i| view.note_rect(&cx.project.melody[i]).is_some_and(|r| p.x > r.right() - 9.0)) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
    }
}

fn toolbar(ui: &mut Ui, cx: &Cx, st: &mut State, r: Rect) {
    let y = r.top();
    if big_button(ui, Rect::from_min_size(pos2(r.left(), y), vec2(190.0, 40.0)), "gen", "Generate a melody", Btn::Primary,
        "Writes a melody over your chords: chord tones on the strong beats, steps in between, a repeating rhythm, a resolving end. Press again for a different one.") {
        st.undo_melody = Some(cx.project.melody.clone());
        st.seed = st.seed.wrapping_add(1);
        let (lo, hi) = cx.p.melody_range();
        let melody = generate_melody(&cx.project, &MelodyCfg { key: cx.key, scale: cx.scale, genre: cx.genre, seed: st.seed, density: cx.p.mel_density.value(), lo, hi });
        cx.p.project.edit(|pr| pr.melody = melody);
    }
    let mut x = r.left() + 200.0;
    if st.undo_melody.is_some() && big_button(ui, Rect::from_min_size(pos2(x, y), vec2(72.0, 40.0)), "undo", "Undo", Btn::Normal, "Restore the melody as it was before the last change.") {
        if let Some(old) = st.undo_melody.take() {
            cx.p.project.edit(|pr| pr.melody = old);
        }
    }
    if st.undo_melody.is_some() {
        x += 78.0;
    }
    if big_button(ui, Rect::from_min_size(pos2(x, y), vec2(72.0, 40.0)), "clear", "Clear", Btn::Quiet, "Remove every melody note.") {
        st.undo_melody = Some(cx.project.melody.clone());
        cx.p.project.edit(|pr| pr.melody.clear());
    }

    // Right side, from the right.
    let mut rx = r.right();
    rx -= 40.0;
    if big_button(ui, Rect::from_min_size(pos2(rx, y), vec2(40.0, 40.0)), "zin", "+", Btn::Normal, "Zoom in") {
        st.zoom = Some((st.zoom.unwrap_or(60.0) * 1.35).min(220.0));
    }
    rx -= 46.0;
    if big_button(ui, Rect::from_min_size(pos2(rx, y), vec2(40.0, 40.0)), "zout", "−", Btn::Normal, "Zoom out") {
        st.zoom = Some((st.zoom.unwrap_or(60.0) / 1.35).max(14.0));
    }
    rx -= 54.0;
    if big_button(ui, Rect::from_min_size(pos2(rx, y), vec2(48.0, 40.0)), "fit", "Fit", Btn::Quiet, "Zoom to show the whole sketch.") {
        st.zoom = None;
        st.roll_x = 0.0;
    }
    rx -= 118.0;
    let snap_names: Vec<&str> = SNAPS.iter().map(|s| s.0).collect();
    if let Some(i) = dd(ui, Rect::from_min_size(pos2(rx, y + 5.0), vec2(112.0, 30.0)), "snap", &format!("Snap: {}", SNAPS[st.snap.min(2)].0), &snap_names, TEXT, "How finely notes snap to the grid.") {
        st.snap = i;
    }
    rx -= 206.0;
    if let Some(i) = segmented(ui, Rect::from_min_size(pos2(rx, y), vec2(198.0, 40.0)), "notes", &["Notes in key", "All notes"], st.show_all as usize, ACCENT) {
        st.show_all = i == 1;
    }
    rx -= 176.0;
    label(ui, pos2(rx, y + 20.0), Align2::LEFT_CENTER, "Busy", 12.5, DIM);
    hslider(ui, cx.setter, &cx.p.mel_density, Rect::from_min_size(pos2(rx + 38.0, y + 9.0), vec2(130.0, 22.0)), AQUA,
        "How busy generated melodies are: few long notes on the left, many short ones on the right.");
}
