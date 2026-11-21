//! The melody piano roll. The harmony is drawn underneath: scale rows are lit, chord tones glow
//! under each chord, and every note is coloured by how it relates to the chord below it
//! (chord tone / scale tone / outside the scale), so you can see what works while you draw.

use super::*;
use crate::generate::*;
use crate::render::voicings;

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

const KEY_W: f32 = 40.0;
const RULER_H: f32 = 14.0;

struct View {
    grid: Rect,
    lo: i32,
    hi: i32,
    row_h: f32,
    ppb: f32,
    scroll: f32,
}

impl View {
    fn x(&self, beat: f32) -> f32 {
        self.grid.left() + beat * self.ppb - self.scroll
    }
    fn beat(&self, x: f32) -> f32 {
        (x - self.grid.left() + self.scroll) / self.ppb
    }
    fn y(&self, pitch: i32) -> f32 {
        self.grid.top() + (self.hi - pitch) as f32 * self.row_h
    }
    fn pitch(&self, y: f32) -> i32 {
        (self.hi - ((y - self.grid.top()) / self.row_h).floor() as i32).clamp(self.lo, self.hi)
    }
    fn note_rect(&self, n: &Note) -> Rect {
        Rect::from_min_size(pos2(self.x(n.start), self.y(n.pitch as i32) + 0.5), vec2((n.len * self.ppb - 1.0).max(3.0), self.row_h - 1.0))
    }
}

fn snap_value(st: &State) -> f32 {
    SNAPS[st.snap.min(SNAPS.len() - 1)].1
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let n_notes = cx.project.melody.len();
    let title = format!("MELODY  ·  PIANO ROLL  ·  {} note{}", n_notes, if n_notes == 1 { "" } else { "s" });
    let inner = panel(ui, rect, &title, AQUA);
    toolbar(ui, cx, st, rect);

    let total_beats = cx.project.beats().max(BEATS_PER_BAR * 2.0);
    let grid_area = Rect::from_min_max(pos2(inner.left() + KEY_W, inner.top() + RULER_H), pos2(inner.right(), inner.bottom()));

    // Visible pitch range: the melody range, the chords and any notes already there.
    let (mlo, mhi) = cx.p.melody_range();
    let mut lo = mlo;
    let mut hi = mhi;
    let voiced = voicings(&cx.project, &cx.cfg);
    for (_, notes) in &voiced {
        for &n in notes {
            lo = lo.min(n as i32);
            hi = hi.max(n as i32);
        }
    }
    for n in &cx.project.melody {
        lo = lo.min(n.pitch as i32);
        hi = hi.max(n.pitch as i32);
    }
    lo = (lo - 2).max(0);
    hi = (hi + 2).min(127);
    while hi - lo + 1 > 36 {
        if (mlo - lo) > (hi - mhi) { lo += 1 } else { hi -= 1 }
    }
    let rows = (hi - lo + 1) as f32;
    let row_h = grid_area.height() / rows;

    let fit = grid_area.width() / total_beats;
    let ppb = st.zoom.unwrap_or(fit).clamp(6.0, 160.0);
    let content_w = total_beats * ppb;
    let max_scroll = (content_w - grid_area.width()).max(0.0);
    if ui.rect_contains_pointer(grid_area) {
        let d = ui.input(|i| i.smooth_scroll_delta);
        st.roll_scroll -= d.x + d.y;
    }
    st.roll_scroll = st.roll_scroll.clamp(0.0, max_scroll);
    let view = View { grid: grid_area, lo, hi, row_h, ppb, scroll: st.roll_scroll };

    let mask = scale_mask(cx.key, cx.scale);
    let painter = ui.painter_at(Rect::from_min_max(pos2(inner.left(), inner.top()), inner.max));
    let grid_painter = ui.painter_at(grid_area);

    // Pitch rows.
    grid_painter.rect_filled(grid_area, 3.0, INSET);
    for pitch in lo..=hi {
        let pc = pitch.rem_euclid(12) as u8;
        let y = view.y(pitch);
        let row = Rect::from_min_size(pos2(grid_area.left(), y), vec2(grid_area.width(), row_h));
        let in_scale = mask[pc as usize];
        let black = matches!(pc, 1 | 3 | 6 | 8 | 10);
        let col = if pc == cx.key { Color32::from_rgb(46, 34, 52) } else if in_scale { Color32::from_rgb(30, 34, 43) } else if black { Color32::from_rgb(11, 12, 15) } else { Color32::from_rgb(15, 17, 21) };
        grid_painter.rect_filled(row, 0.0, col);
    }
    // Time grid.
    let first_beat = (view.scroll / ppb).floor() as i32;
    let last_beat = ((view.scroll + grid_area.width()) / ppb).ceil() as i32;
    for b in first_beat..=last_beat {
        let x = view.x(b as f32);
        let bar = b % 4 == 0;
        grid_painter.line_segment([pos2(x, grid_area.top()), pos2(x, grid_area.bottom())], st_s(1.0, if bar { Color32::from_rgb(52, 58, 70) } else { Color32::from_rgb(26, 29, 36) }));
        if bar && b >= 0 {
            painter.text(pos2(x + 3.0, inner.top() + 6.0), Align2::LEFT_CENTER, format!("{}", b / 4 + 1), FontId::proportional(8.5), FAINT);
        }
    }

    // Chord overlays: glowing chord-tone rows plus the ghosted voicing and the chord name.
    for (p, notes) in &voiced {
        let (x0, x1) = (view.x(p.start), view.x(p.start + p.item_beats));
        if x1 < grid_area.left() || x0 > grid_area.right() {
            continue;
        }
        let pcs = p.chord.pitch_classes();
        let col = kind_color(p.chord.kind());
        for pitch in lo..=hi {
            if pcs.contains(&(pitch.rem_euclid(12) as u8)) {
                let r = Rect::from_min_max(pos2(x0.max(grid_area.left()), view.y(pitch)), pos2((x1 - 1.0).min(grid_area.right()), view.y(pitch) + row_h));
                grid_painter.rect_filled(r, 0.0, col.gamma_multiply(0.13));
            }
        }
        for &n in notes {
            let r = Rect::from_min_max(pos2(x0 + 1.0, view.y(n as i32) + 1.0), pos2(x1 - 2.0, view.y(n as i32) + row_h - 1.0));
            grid_painter.rect_stroke(r, 1.0, st_s(1.0, col.gamma_multiply(0.55)), StrokeKind::Inside);
        }
        grid_painter.line_segment([pos2(x0, grid_area.top()), pos2(x0, grid_area.bottom())], st_s(1.0, col.gamma_multiply(0.5)));
        painter.text(pos2((x0 + 4.0).max(grid_area.left() + 2.0), inner.top() + 6.0), Align2::LEFT_CENTER, p.chord.name(cx.flats), FontId::proportional(9.5), col);
    }

    // Keys on the left.
    key_strip(ui, cx, st, &view, inner, &mask);

    // Melody notes.
    for (i, n) in cx.project.melody.iter().enumerate() {
        let r = view.note_rect(n);
        if r.right() < grid_area.left() || r.left() > grid_area.right() || n.pitch as i32 > hi || (n.pitch as i32) < lo {
            continue;
        }
        let pc = n.pitch % 12;
        let chord = cx.project.chord_at(n.start + 0.001).map(|p| p.chord);
        let (col, kind) = if !mask[pc as usize] {
            (Color32::from_rgb(255, 120, 90), "outside the scale")
        } else if chord.map_or(false, |c| c.contains_pc(pc)) {
            (AQUA, "chord tone")
        } else {
            (MINOR_C, "scale tone (passing)")
        };
        let _ = kind;
        let selected = st.roll.sel == Some(i);
        grid_painter.rect_filled(r, 2.0, col.gamma_multiply(0.45 + 0.4 * (n.vel as f32 / 127.0)));
        grid_painter.rect_stroke(r, 2.0, st_s(if selected { 1.8 } else { 1.0 }, if selected { Color32::WHITE } else { col }), StrokeKind::Inside);
    }

    // Playhead.
    let head = cx.shared.playhead.load(Ordering::Relaxed);
    if head >= 0.0 {
        let x = view.x(head);
        grid_painter.line_segment([pos2(x, grid_area.top()), pos2(x, grid_area.bottom())], st_s(1.5, Color32::WHITE));
    }

    interact(ui, cx, st, &view, total_beats);

    // Legend.
    let ly = rect.bottom() - 11.0;
    let mut lx = inner.left() + KEY_W + 6.0;
    for (c, label) in [(AQUA, "chord tone"), (MINOR_C, "scale tone"), (Color32::from_rgb(255, 120, 90), "outside the scale")] {
        ui.painter().rect_filled(Rect::from_center_size(pos2(lx, ly), vec2(8.0, 6.0)), 1.0, c);
        ui.painter().text(pos2(lx + 7.0, ly), Align2::LEFT_CENTER, label, FontId::proportional(8.5), DIM);
        lx += 24.0 + label.len() as f32 * 4.6;
    }
    ui.painter().text(pos2(lx + 10.0, ly), Align2::LEFT_CENTER, "click = add note · drag = move · drag right edge = length · right-click = delete · wheel = scroll", FontId::proportional(8.5), FAINT);
}

fn st_s(w: f32, c: Color32) -> egui::Stroke {
    plugin_common::ui::st(w, c)
}

fn key_strip(ui: &mut Ui, cx: &Cx, st: &mut State, view: &View, inner: Rect, mask: &[bool; 12]) {
    let strip = Rect::from_min_max(pos2(inner.left(), view.grid.top()), pos2(inner.left() + KEY_W - 2.0, view.grid.bottom()));
    let painter = ui.painter_at(strip);
    for pitch in view.lo..=view.hi {
        let pc = pitch.rem_euclid(12) as u8;
        let y = view.y(pitch);
        let black = matches!(pc, 1 | 3 | 6 | 8 | 10);
        let col = if black { Color32::from_rgb(20, 22, 27) } else { Color32::from_rgb(64, 69, 82) };
        let r = Rect::from_min_size(pos2(strip.left(), y), vec2(strip.width(), view.row_h));
        painter.rect_filled(r.shrink2(vec2(0.0, 0.3)), 0.0, col);
        if mask[pc as usize] {
            painter.rect_filled(Rect::from_min_size(pos2(strip.left(), y + 0.5), vec2(3.0, view.row_h - 1.0)), 0.0, if pc == cx.key { ACCENT } else { AQUA.gamma_multiply(0.7) });
        }
        if pc == 0 || pc == cx.key || view.row_h >= 9.0 {
            let label = if pc == 0 { format!("C{}", pitch / 12 - 1) } else if pc == cx.key && view.row_h < 9.0 { pc_name(pc, cx.flats).to_string() } else if view.row_h >= 9.0 { pc_name(pc, cx.flats).to_string() } else { String::new() };
            painter.text(pos2(strip.right() - 3.0, y + view.row_h / 2.0), Align2::RIGHT_CENTER, label, FontId::proportional(7.5), if black { FAINT } else { Color32::from_rgb(20, 22, 27) });
        }
    }
    // Press a key to hear the pitch.
    let resp = ui.interact(strip, Id::new("roll-keys"), Sense::click_and_drag());
    let down = resp.is_pointer_button_down_on();
    let hit = if down { resp.interact_pointer_pos().map(|p| view.pitch(p.y) as u8) } else { None };
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

fn interact(ui: &mut Ui, cx: &Cx, st: &mut State, view: &View, total_beats: f32) {
    let resp = ui.interact(view.grid, Id::new("roll-grid"), Sense::click_and_drag());
    let snap = snap_value(st);
    let pos = ui.input(|i| i.pointer.interact_pos());
    let pressed = ui.input(|i| i.pointer.primary_pressed());
    let down = ui.input(|i| i.pointer.primary_down());

    let hit_note = |p: egui::Pos2| cx.project.melody.iter().enumerate().rev().find(|(_, n)| view.note_rect(n).expand2(vec2(1.0, 0.0)).contains(p)).map(|(i, _)| i);

    if resp.secondary_clicked() {
        if let Some(i) = pos.and_then(hit_note) {
            let note = cx.project.melody[i];
            st.undo_melody = Some(cx.project.melody.clone());
            cx.p.project.edit(|pr| {
                if i < pr.melody.len() {
                    pr.melody.remove(i);
                }
            });
            st.roll.sel = None;
            let _ = note;
        }
    }

    if pressed && resp.hovered() {
        if let Some(p) = pos {
            let beat = view.beat(p.x);
            let pitch = view.pitch(p.y);
            match hit_note(p) {
                Some(i) => {
                    let n = cx.project.melody[i];
                    st.roll.sel = Some(i);
                    let edge = view.note_rect(&n).right();
                    if p.x > edge - 6.0 && n.len * view.ppb > 14.0 {
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
        let pitch = view.pitch(p.y) as u8;
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
        // Keep the list tidy, keeping the selection on the same note.
        cx.p.project.edit(|pr| pr.melody.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch))));
        st.roll.sel = None;
    }
    if resp.hovered() || st.roll.drag.is_some() {
        if let Some(p) = pos {
            if hit_note(p).map_or(false, |i| p.x > view.note_rect(&cx.project.melody[i]).right() - 6.0) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
    }
    if let Some(i) = st.roll.sel {
        if ui.input(|i| i.key_pressed(egui::Key::Delete)) && st.roll.drag.is_none() {
            cx.p.project.edit(|pr| {
                if i < pr.melody.len() {
                    pr.melody.remove(i);
                }
            });
            st.roll.sel = None;
        }
    }
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
    if button(ui, place(54.0), "mel-clear", "CLEAR", DIM, "Remove every melody note.") {
        st.undo_melody = Some(cx.project.melody.clone());
        cx.p.project.edit(|pr| pr.melody.clear());
    }
    if button(ui, place(88.0), "mel-fix", "SNAP TO KEY", DIM, "Move every note that is outside the scale to the nearest scale note.") {
        st.undo_melody = Some(cx.project.melody.clone());
        let (key, scale) = (cx.key, cx.scale);
        cx.p.project.edit(|pr| {
            for n in &mut pr.melody {
                if !in_scale(key, scale, n.pitch % 12) {
                    let up = (1..=2).find(|d| in_scale(key, scale, ((n.pitch as i32 + d) % 12) as u8)).unwrap_or(1);
                    let down = (1..=2).find(|d| in_scale(key, scale, ((n.pitch as i32 - d).rem_euclid(12)) as u8)).unwrap_or(1);
                    n.pitch = if down < up { n.pitch.saturating_sub(down as u8) } else { n.pitch.saturating_add(up as u8) };
                }
            }
        });
    }
    if st.undo_melody.is_some() && button(ui, place(50.0), "mel-undo", "UNDO", AMBER, "Restore the melody as it was before the last generate / clear / edit.") {
        if let Some(old) = st.undo_melody.take() {
            cx.p.project.edit(|pr| pr.melody = old);
        }
    }
    if button(ui, place(86.0), "mel-gen", "GENERATE", ACCENT,
        "Write a new melody over the chords: chord tones on the strong beats, stepwise motion in between, a repeating rhythm and a resolving end. Press again for another one.") {
        st.undo_melody = Some(cx.project.melody.clone());
        st.seed = st.seed.wrapping_add(1);
        let (lo, hi) = cx.p.melody_range();
        let melody = generate_melody(&cx.project, &MelodyCfg { key: cx.key, scale: cx.scale, genre: cx.genre, seed: st.seed, density: cx.p.mel_density.value(), lo, hi });
        cx.p.project.edit(|pr| pr.melody = melody);
    }
    // Zoom, snap, generator settings (left of the buttons).
    if button(ui, place(34.0), "roll-fit", "FIT", DIM, "Zoom to show the whole sketch.") {
        st.zoom = None;
        st.roll_scroll = 0.0;
    }
    let ppb_now = st.zoom.unwrap_or(1.0);
    if button(ui, place(24.0), "roll-zin", "+", TEXT, "Zoom in") {
        st.zoom = Some((st.zoom.unwrap_or(fit_ppb(cx, rect)) * 1.4).min(160.0));
    }
    if button(ui, place(24.0), "roll-zout", "−", TEXT, "Zoom out") {
        st.zoom = Some((st.zoom.unwrap_or(fit_ppb(cx, rect)) / 1.4).max(6.0));
    }
    let _ = ppb_now;
    let snap_label = format!("Snap {}", SNAPS[st.snap.min(2)].0);
    let snaps: Vec<&str> = SNAPS.iter().map(|s| s.0).collect();
    if let Some(i) = dd(ui, place(78.0), "snap", &snap_label, &snaps, DIM, "Grid for drawing and moving notes.") {
        st.snap = i;
    }
    let dens_rect = place(96.0);
    hslider(ui, cx.setter, &cx.p.mel_density, dens_rect, AQUA, "How busy generated melodies are: sparse and long notes on the left, busy runs on the right.");
    let label_rect = place(52.0);
    ui.painter().text(label_rect.center(), Align2::CENTER_CENTER, "DENSITY", FontId::proportional(8.5), FAINT);
    let octs = ["Melody −1 oct", "Melody 0", "Melody +1 oct", "Melody +2 oct"];
    let oct_label = octs[(cx.p.mel_octave.value() + 1).clamp(0, 3) as usize];
    if let Some(i) = dd(ui, place(104.0), "mel-oct", oct_label, &octs, DIM, "Register of generated melodies.") {
        set(cx.setter, &cx.p.mel_octave, i as i32 - 1);
    }
}

fn fit_ppb(cx: &Cx, rect: Rect) -> f32 {
    let width = rect.width() - 20.0 - KEY_W;
    width / cx.project.beats().max(BEATS_PER_BAR * 2.0)
}
