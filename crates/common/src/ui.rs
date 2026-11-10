//! Shared look and widgets for every plugin editor.
//!
//! Everything is drawn with explicit rectangles (no auto layout), so panels line up on a fixed
//! grid. The palette is deliberately small: neutral greys plus the accent colours each plugin
//! passes in. Every control takes a `tip` that is shown, with the live value, when hovered.

use crate::midi::{is_standalone, KeyState, MidiQueue};
use nih_plug::prelude::*;
use nih_plug_egui::egui::{
    self, pos2, vec2, Align2, Color32, FontId, Id, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui,
};
use std::f32::consts::PI;

// ---------------------------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------------------------

pub const BG: Color32 = Color32::from_rgb(15, 17, 21);
pub const PANEL: Color32 = Color32::from_rgb(26, 29, 35);
pub const RAISED: Color32 = Color32::from_rgb(38, 42, 51);
pub const RAISED_HOVER: Color32 = Color32::from_rgb(50, 55, 66);
pub const INSET: Color32 = Color32::from_rgb(10, 11, 14);
pub const LINE: Color32 = Color32::from_rgb(44, 49, 59);
pub const TRACK: Color32 = Color32::from_rgb(54, 59, 71);
pub const TEXT: Color32 = Color32::from_rgb(230, 233, 240);
pub const DIM: Color32 = Color32::from_rgb(138, 145, 160);
pub const FAINT: Color32 = Color32::from_rgb(86, 92, 106);
pub const WARN: Color32 = Color32::from_rgb(255, 176, 64);
pub const HOT: Color32 = Color32::from_rgb(255, 86, 86);

pub fn st(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

/// Apply the shared dark style (popups, tooltips, selection colour) to an egui context.
pub fn apply_style(ctx: &egui::Context, accent: Color32) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = Color32::from_rgb(32, 36, 44);
    v.window_stroke = st(1.0, LINE);
    v.override_text_color = Some(TEXT);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = RAISED;
    v.widgets.hovered.weak_bg_fill = RAISED_HOVER;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.weak_bg_fill = RAISED_HOVER;
    v.selection.bg_fill = accent.gamma_multiply(0.35);
    v.selection.stroke = st(1.0, accent);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.interaction.tooltip_delay = 0.35;
        s.spacing.item_spacing = vec2(6.0, 3.0);
    });
}

// ---------------------------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------------------------

pub fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let steps = ((a1 - a0).abs() * r / 2.0).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Text with extra letter spacing, used for section titles.
pub fn spaced_text(painter: &Painter, pos: Pos2, text: &str, size: f32, color: Color32, tracking: f32) -> f32 {
    let mut x = pos.x;
    for ch in text.chars() {
        let galley = painter.layout_no_wrap(ch.to_string(), FontId::proportional(size), color);
        let w = galley.size().x;
        painter.galley(pos2(x, pos.y - galley.size().y * 0.5), galley, color);
        x += w + tracking;
    }
    x - pos.x
}

/// Fill the area between a curve and `base_y` with a vertical fade.
pub fn fill_under(painter: &Painter, pts: &[Pos2], base_y: f32, color: Color32, alpha: f32) {
    if pts.len() < 2 {
        return;
    }
    let top = color.gamma_multiply(alpha);
    let bottom = color.gamma_multiply(0.0);
    let mut mesh = Mesh::default();
    for p in pts {
        mesh.colored_vertex(*p, top);
        mesh.colored_vertex(pos2(p.x, base_y), bottom);
    }
    for i in 0..pts.len() as u32 - 1 {
        let a = 2 * i;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// A recessed screen for waveforms and curves. Returns a painter clipped to it.
pub fn display(ui: &Ui, rect: Rect) -> Painter {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, INSET);
    painter.rect_stroke(rect, 6.0, st(1.0, LINE), StrokeKind::Inside);
    painter
}

/// Faint grid lines inside a display.
pub fn grid(painter: &Painter, rect: Rect, cols: usize, rows: usize) {
    let c = Color32::from_rgb(24, 27, 33);
    for i in 1..cols {
        let x = rect.left() + rect.width() * i as f32 / cols as f32;
        painter.line_segment([pos2(x, rect.top() + 1.0), pos2(x, rect.bottom() - 1.0)], st(1.0, c));
    }
    for i in 1..rows {
        let y = rect.top() + rect.height() * i as f32 / rows as f32;
        painter.line_segment([pos2(rect.left() + 1.0, y), pos2(rect.right() - 1.0, y)], st(1.0, c));
    }
}

fn tooltip<P: Param>(resp: Response, p: &P, tip: &str, hint: &str) -> Response {
    resp.on_hover_ui(|ui| {
        ui.set_max_width(270.0);
        let value = p.normalized_value_to_string(p.unmodulated_normalized_value(), true);
        ui.label(egui::RichText::new(format!("{}   {}", p.name(), value)).color(TEXT).size(12.5));
        if !tip.is_empty() {
            ui.label(egui::RichText::new(tip).color(DIM).size(11.5));
        }
        if !hint.is_empty() {
            ui.label(egui::RichText::new(hint).color(FAINT).size(10.5));
        }
    })
}

fn param_id<P: Param>(p: &P) -> Id {
    Id::new(("param", p as *const P as usize))
}

fn set_norm<P: Param>(setter: &ParamSetter, p: &P, norm: f32) {
    setter.begin_set_parameter(p);
    setter.set_parameter_normalized(p, norm);
    setter.end_set_parameter(p);
}

// ---------------------------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------------------------

pub const TITLE_H: f32 = 26.0;

/// Draw a panel with a title and return the content rectangle below the title bar.
pub fn panel(ui: &Ui, rect: Rect, title: &str, color: Color32) -> Rect {
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, PANEL);
    painter.rect_filled(
        Rect::from_min_size(pos2(rect.left() + 12.0, rect.top() + 9.0), vec2(3.0, 9.0)),
        1.5,
        color,
    );
    spaced_text(painter, pos2(rect.left() + 21.0, rect.top() + 13.5), title, 10.5, TEXT, 1.4);
    Rect::from_min_max(pos2(rect.left() + 10.0, rect.top() + TITLE_H), pos2(rect.right() - 10.0, rect.bottom() - 8.0))
}

/// Small caption inside a panel.
pub fn caption(ui: &Ui, pos: Pos2, text: &str) {
    spaced_text(ui.painter(), pos, text, 9.0, FAINT, 1.0);
}

// ---------------------------------------------------------------------------------------------
// Knob
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Kn {
    /// Small: secondary controls.
    S,
    /// Medium: the default.
    M,
    /// Large: the one or two controls that define a section.
    L,
}

impl Kn {
    pub fn radius(self) -> f32 {
        match self {
            Kn::S => 10.0,
            Kn::M => 14.0,
            Kn::L => 21.0,
        }
    }
}

/// A rotary control centred on `c`, with its label underneath. While hovered or dragged the label
/// is replaced by the current value.
pub fn knob<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    p: &P,
    c: Pos2,
    size: Kn,
    label: &str,
    color: Color32,
    tip: &str,
) -> Response {
    let r = size.radius();
    let rect = Rect::from_min_max(pos2(c.x - r - 12.0, c.y - r - 6.0), pos2(c.x + r + 12.0, c.y + r + 19.0));
    let resp = ui.interact(rect, param_id(p), Sense::click_and_drag());
    let mut norm = p.unmodulated_normalized_value();

    if resp.drag_started() {
        setter.begin_set_parameter(p);
    }
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        let speed = if fine { 0.0006 } else { 0.005 };
        norm = (norm - resp.drag_delta().y * speed).clamp(0.0, 1.0);
        setter.set_parameter_normalized(p, norm);
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(p);
    }
    let ctrl = ui.input(|i| i.modifiers.command);
    if resp.double_clicked() || (resp.clicked() && ctrl) {
        set_norm(setter, p, p.default_normalized_value());
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let step = match p.step_count() {
                Some(n) if n > 0 && n <= 64 => scroll.signum() / n as f32,
                _ => scroll * 0.0012,
            };
            set_norm(setter, p, (norm + step).clamp(0.0, 1.0));
        }
    }

    let norm = p.unmodulated_normalized_value();
    let active = resp.hovered() || resp.dragged();
    let painter = ui.painter();
    let a0 = 0.75 * PI;
    let sweep = 1.5 * PI;
    let ring = r + 4.0;
    let w = if size == Kn::L { 3.0 } else { 2.5 };

    painter.add(Shape::line(arc_points(c, ring, a0, a0 + sweep), st(w, TRACK)));
    let bipolar = (p.default_normalized_value() - 0.5).abs() < 0.01 && p.step_count().is_none();
    let from = if bipolar { 0.5 } else { 0.0 };
    if (norm - from).abs() > 0.003 {
        let (lo, hi) = if from < norm { (from, norm) } else { (norm, from) };
        painter.add(Shape::line(arc_points(c, ring, a0 + sweep * lo, a0 + sweep * hi), st(w, color)));
    }
    // Body: a flat cap with a soft top highlight and a thin dark rim.
    painter.circle_filled(c, r, if active { RAISED_HOVER } else { RAISED });
    painter.circle_stroke(c, r, st(1.0, Color32::from_rgb(12, 13, 16)));
    painter.add(Shape::line(
        arc_points(c, r - 1.5, 1.15 * PI, 1.85 * PI),
        st(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 22)),
    ));
    let ang = a0 + sweep * norm;
    let dir = vec2(ang.cos(), ang.sin());
    painter.line_segment([c + dir * (r * 0.38), c + dir * (r - 2.5)], st(2.0, if active { color } else { TEXT }));

    let font = FontId::proportional(if size == Kn::S { 9.0 } else { 9.5 });
    let text_pos = pos2(c.x, c.y + r + 12.0);
    if active {
        painter.text(text_pos, Align2::CENTER_CENTER, p.normalized_value_to_string(norm, true), font, TEXT);
    } else {
        painter.text(text_pos, Align2::CENTER_CENTER, label, font, DIM);
    }
    tooltip(resp, p, tip, "Drag up/down  ·  Shift = fine  ·  Double-click = reset  ·  Wheel")
}

// ---------------------------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------------------------

/// A pill-shaped on/off switch with a text label.
pub fn toggle(ui: &mut Ui, setter: &ParamSetter, p: &BoolParam, rect: Rect, label: &str, color: Color32, tip: &str) {
    let resp = ui.interact(rect, param_id(p), Sense::click());
    let on = p.value();
    if resp.clicked() {
        setter.begin_set_parameter(p);
        setter.set_parameter(p, !on);
        setter.end_set_parameter(p);
    }
    let painter = ui.painter();
    let radius = rect.height() * 0.5;
    if on {
        painter.rect_filled(rect, radius, color.gamma_multiply(0.18));
        painter.rect_stroke(rect, radius, st(1.0, color.gamma_multiply(0.8)), StrokeKind::Inside);
    } else {
        painter.rect_filled(rect, radius, if resp.hovered() { RAISED_HOVER } else { RAISED });
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(9.5), if on { color } else { DIM });
    tooltip(resp, p, tip, "Click to switch");
}

/// A round power button, used to switch a whole section on or off.
pub fn power(ui: &mut Ui, setter: &ParamSetter, p: &BoolParam, c: Pos2, color: Color32, tip: &str) {
    let rect = Rect::from_center_size(c, vec2(18.0, 18.0));
    let resp = ui.interact(rect, param_id(p), Sense::click());
    let on = p.value();
    if resp.clicked() {
        setter.begin_set_parameter(p);
        setter.set_parameter(p, !on);
        setter.end_set_parameter(p);
    }
    let painter = ui.painter();
    let col = if on { color } else if resp.hovered() { DIM } else { FAINT };
    painter.add(Shape::line(arc_points(c, 5.5, -0.25 * PI, 1.25 * PI), st(1.6, col)));
    painter.line_segment([pos2(c.x, c.y - 7.0), pos2(c.x, c.y - 1.5)], st(1.6, col));
    tooltip(resp, p, tip, "Click to switch on / off");
}

/// A plain clickable button (not bound to a parameter). Returns true when clicked.
pub fn button(ui: &mut Ui, rect: Rect, id: &str, label: &str, color: Color32, tip: &str) -> bool {
    let resp = ui.interact(rect, Id::new(("btn", id)), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, if resp.hovered() { RAISED_HOVER } else { RAISED });
    painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(11.0), color);
    let clicked = resp.clicked();
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    clicked
}

fn chevron(painter: &Painter, c: Pos2, color: Color32) {
    painter.add(Shape::line(vec![c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)], st(1.4, color)));
}

/// A drop-down list for an enum (or any stepped) parameter. Scrolling over it steps the value.
pub fn dropdown<P: Param>(ui: &mut Ui, setter: &ParamSetter, p: &P, rect: Rect, tip: &str) {
    let id = param_id(p);
    let popup_id = id.with("popup");
    let resp = ui.interact(rect, id, Sense::click());
    let steps = p.step_count().unwrap_or(0).max(1);
    let cur = p.unmodulated_normalized_value();
    let cur_idx = (cur * steps as f32).round() as usize;
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let next = (cur_idx as i32 - scroll.signum() as i32).clamp(0, steps as i32) as usize;
            if next != cur_idx {
                set_norm(setter, p, next as f32 / steps as f32);
            }
        }
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, if resp.hovered() { RAISED_HOVER } else { RAISED });
    let clip = Rect::from_min_max(rect.min, pos2(rect.right() - 16.0, rect.bottom()));
    ui.painter_at(clip).text(
        pos2(rect.left() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        p.normalized_value_to_string(cur, false),
        FontId::proportional(10.5),
        TEXT,
    );
    chevron(painter, pos2(rect.right() - 10.0, rect.center().y), DIM);

    egui::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(rect.width().max(130.0));
        egui::ScrollArea::vertical().max_height(330.0).show(ui, |ui| {
            for i in 0..=steps {
                let nv = i as f32 / steps as f32;
                let text = egui::RichText::new(p.normalized_value_to_string(nv, false)).size(11.5);
                if ui.selectable_label(i == cur_idx, text).clicked() {
                    set_norm(setter, p, nv);
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        });
    });
    tooltip(resp, p, tip, "Click to choose  ·  Wheel to step");
}

/// Horizontal slider for bipolar amounts (modulation depth). Fills from the centre.
pub fn hslider<P: Param>(ui: &mut Ui, setter: &ParamSetter, p: &P, rect: Rect, color: Color32, tip: &str) {
    let resp = ui.interact(rect, param_id(p), Sense::click_and_drag());
    let mut norm = p.unmodulated_normalized_value();
    if resp.drag_started() {
        setter.begin_set_parameter(p);
    }
    if resp.dragged() {
        let fine = ui.input(|i| i.modifiers.shift);
        norm = (norm + resp.drag_delta().x / rect.width() * if fine { 0.1 } else { 1.0 }).clamp(0.0, 1.0);
        setter.set_parameter_normalized(p, norm);
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(p);
    }
    if resp.double_clicked() {
        set_norm(setter, p, p.default_normalized_value());
    }
    let norm = p.unmodulated_normalized_value();
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, INSET);
    let mid = rect.center().x;
    let x = rect.left() + rect.width() * norm;
    let (a, b) = if x < mid { (x, mid) } else { (mid, x) };
    if b - a > 0.5 {
        painter.rect_filled(
            Rect::from_min_max(pos2(a, rect.top() + 2.0), pos2(b, rect.bottom() - 2.0)),
            2.0,
            color.gamma_multiply(0.55),
        );
    }
    painter.line_segment([pos2(mid, rect.top() + 2.0), pos2(mid, rect.bottom() - 2.0)], st(1.0, FAINT));
    painter.line_segment([pos2(x, rect.top() + 1.0), pos2(x, rect.bottom() - 1.0)], st(2.0, color));
    painter.text(
        pos2(rect.right() - 5.0, rect.center().y),
        Align2::RIGHT_CENTER,
        p.normalized_value_to_string(norm, false),
        FontId::proportional(9.5),
        if resp.hovered() || resp.dragged() { TEXT } else { DIM },
    );
    tooltip(resp, p, tip, "Drag left/right  ·  Shift = fine  ·  Double-click = reset");
}

// ---------------------------------------------------------------------------------------------
// Header pieces
// ---------------------------------------------------------------------------------------------

/// Output level meter with a dB scale.
pub fn meter(ui: &Ui, rect: Rect, peak: f32, color: Color32) {
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, INSET);
    let db = util::gain_to_db(peak.max(1e-5));
    let frac = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
    let segs = 24;
    let gap = 1.5;
    let w = (rect.width() - 4.0 - gap * (segs - 1) as f32) / segs as f32;
    for i in 0..segs {
        let t = (i + 1) as f32 / segs as f32;
        let lit = frac >= t - 0.5 / segs as f32;
        let c = if t > 0.96 { HOT } else if t > 0.8 { WARN } else { color };
        let x = rect.left() + 2.0 + i as f32 * (w + gap);
        painter.rect_filled(
            Rect::from_min_size(pos2(x, rect.top() + 2.0), vec2(w, rect.height() - 4.0)),
            1.0,
            if lit { c } else { c.gamma_multiply(0.12) },
        );
    }
}

/// Preset browser: previous / name with a categorised list / next. Returns the index to load.
///
/// `user` is the name of a loaded user preset (from a file); while it is set the bar shows that
/// name, and previous / next step back into the factory list.
pub fn preset_bar(
    ui: &mut Ui,
    rect: Rect,
    presets: &[(&str, &str)],
    current: usize,
    user: Option<&str>,
    color: Color32,
) -> Option<usize> {
    let n = presets.len();
    let current = current.min(n - 1);
    let (shown_name, shown_category) = match user {
        Some(name) => (name, "User"),
        None => presets[current],
    };
    let mut pick = None;
    let bw = 26.0;
    let prev = Rect::from_min_size(rect.min, vec2(bw, rect.height()));
    let next = Rect::from_min_size(pos2(rect.right() - bw, rect.top()), vec2(bw, rect.height()));
    let name = Rect::from_min_max(pos2(prev.right() + 4.0, rect.top()), pos2(next.left() - 4.0, rect.bottom()));
    if button(ui, prev, "preset-prev", "‹", TEXT, "Previous preset") {
        pick = Some((current + n - 1) % n);
    }
    if button(ui, next, "preset-next", "›", TEXT, "Next preset") {
        pick = Some((current + 1) % n);
    }

    let id = Id::new("preset-name");
    let popup_id = id.with("popup");
    let resp = ui.interact(name, id, Sense::click());
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            pick = Some(if scroll < 0.0 { (current + 1) % n } else { (current + n - 1) % n });
        }
    }
    let painter = ui.painter();
    painter.rect_filled(name, 5.0, INSET);
    painter.rect_stroke(name, 5.0, st(1.0, if resp.hovered() { color.gamma_multiply(0.6) } else { LINE }), StrokeKind::Inside);
    spaced_text(painter, pos2(name.left() + 10.0, name.center().y), &shown_category.to_uppercase(), 9.0, color, 1.0);
    let clip = Rect::from_min_max(pos2(name.left() + 74.0, name.top()), pos2(name.right() - 22.0, name.bottom()));
    ui.painter_at(clip).text(clip.center(), Align2::CENTER_CENTER, shown_name, FontId::proportional(13.0), TEXT);
    chevron(painter, pos2(name.right() - 12.0, name.center().y), DIM);

    egui::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(name.width());
        egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
            let mut last = "";
            for (i, (title, category)) in presets.iter().enumerate() {
                if *category != last {
                    if !last.is_empty() {
                        ui.add_space(4.0);
                    }
                    ui.label(egui::RichText::new(category.to_uppercase()).size(9.5).color(color));
                    last = category;
                }
                if ui.selectable_label(i == current && user.is_none(), egui::RichText::new(*title).size(12.0)).clicked() {
                    pick = Some(i);
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        });
    });
    resp.on_hover_text("Factory presets. Click for the list, or scroll to step through them.\nLoading a preset replaces every setting.");
    pick
}

/// The latest preset load / save message, shown for a few seconds.
pub fn io_status(ui: &Ui, io: &crate::preset_io::PresetIo) -> Option<String> {
    let id = Id::new("preset-io-status");
    let now = ui.input(|i| i.time);
    if let Some(message) = io.take_status() {
        ui.data_mut(|d| d.insert_temp(id, (message, now)));
    }
    let shown: Option<(String, f64)> = ui.data(|d| d.get_temp(id));
    shown.filter(|(_, at)| now - at < 5.0).map(|(message, _)| message)
}

// ---------------------------------------------------------------------------------------------
// On-screen keyboard
// ---------------------------------------------------------------------------------------------

const KEY_LOW: u8 = 24; // C1
const WHITE_KEYS: usize = 42; // six octaves
const WHITE_SEMIS: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Computer keys mapped to one and a half octaves, like a tracker / DAW "musical typing" layout.
const TYPING: [(egui::Key, u8); 18] = [
    (egui::Key::A, 0),
    (egui::Key::W, 1),
    (egui::Key::S, 2),
    (egui::Key::E, 3),
    (egui::Key::D, 4),
    (egui::Key::F, 5),
    (egui::Key::T, 6),
    (egui::Key::G, 7),
    (egui::Key::Y, 8),
    (egui::Key::H, 9),
    (egui::Key::U, 10),
    (egui::Key::J, 11),
    (egui::Key::K, 12),
    (egui::Key::O, 13),
    (egui::Key::L, 14),
    (egui::Key::P, 15),
    (egui::Key::Semicolon, 16),
    (egui::Key::Quote, 17),
];

#[derive(Clone, Copy)]
struct KbState {
    mouse_note: i32,
    typing_mask: u32,
    octave: i32,
}

impl Default for KbState {
    fn default() -> Self {
        Self { mouse_note: -1, typing_mask: 0, octave: 3 }
    }
}

/// A playable piano strip. Clicking (or dragging across) keys sends notes into `queue`; keys held
/// by any source light up from `held`. In the standalone app the computer keyboard plays too.
pub fn keyboard(ui: &mut Ui, rect: Rect, queue: &MidiQueue, held: &KeyState, color: Color32) {
    let id = Id::new("piano");
    let mut state: KbState = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let kw = rect.width() / WHITE_KEYS as f32;
    let black_h = rect.height() * 0.62;
    let black_w = kw * 0.62;

    let white_note = |i: usize| KEY_LOW + 12 * (i / 7) as u8 + WHITE_SEMIS[i % 7];
    let has_black = |i: usize| !matches!(i % 7, 2 | 6); // no black key after E and B

    // Which note is under the pointer?
    let mut hit: i32 = -1;
    let mut vel = 0.8;
    if resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let x = (pos.x - rect.left()).clamp(0.0, rect.width() - 0.01);
            let wi = (x / kw) as usize;
            hit = white_note(wi) as i32;
            vel = (0.35 + 0.65 * (pos.y - rect.top()) / rect.height()).clamp(0.2, 1.0);
            if pos.y < rect.top() + black_h {
                let off = x - wi as f32 * kw;
                if off > kw - black_w * 0.5 && has_black(wi) && wi + 1 < WHITE_KEYS {
                    hit = white_note(wi) as i32 + 1;
                } else if off < black_w * 0.5 && wi > 0 && has_black(wi - 1) {
                    hit = white_note(wi - 1) as i32 + 1;
                }
            }
        }
    }
    if hit != state.mouse_note {
        if state.mouse_note >= 0 {
            queue.push(0x80, state.mouse_note as u8, 0);
        }
        if hit >= 0 {
            queue.push(0x90, hit as u8, (vel * 127.0) as u8);
        }
        state.mouse_note = hit;
    }

    // Musical typing (standalone only, so a DAW's own shortcuts keep working).
    if is_standalone() {
        let (mut mask, mut shift) = (0u32, 0i32);
        ui.input(|i| {
            for (bit, (key, _)) in TYPING.iter().enumerate() {
                if i.key_down(*key) {
                    mask |= 1 << bit;
                }
            }
            if i.key_pressed(egui::Key::Z) {
                shift -= 1;
            }
            if i.key_pressed(egui::Key::X) {
                shift += 1;
            }
        });
        let base = |oct: i32| 12 * (oct + 1);
        if shift != 0 {
            // Release everything before moving, so no note is left hanging in the old octave.
            for (bit, (_, semi)) in TYPING.iter().enumerate() {
                if state.typing_mask & (1 << bit) != 0 {
                    queue.push(0x80, (base(state.octave) + *semi as i32) as u8, 0);
                }
            }
            state.typing_mask = 0;
            state.octave = (state.octave + shift).clamp(0, 7);
        }
        for (bit, (_, semi)) in TYPING.iter().enumerate() {
            let (was, is) = (state.typing_mask & (1 << bit) != 0, mask & (1 << bit) != 0);
            let note = (base(state.octave) + *semi as i32).clamp(0, 127) as u8;
            if is && !was {
                queue.push(0x90, note, 100);
            } else if was && !is {
                queue.push(0x80, note, 0);
            }
        }
        state.typing_mask = mask;
    }
    ui.data_mut(|d| d.insert_temp(id, state));

    // Draw
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, INSET);
    let lit = |note: u8| held.is_on(note) || state.mouse_note == note as i32;
    for i in 0..WHITE_KEYS {
        let note = white_note(i);
        let r = Rect::from_min_max(
            pos2(rect.left() + i as f32 * kw + 0.5, rect.top()),
            pos2(rect.left() + (i + 1) as f32 * kw - 0.5, rect.bottom()),
        );
        let fill = if lit(note) { color } else { Color32::from_rgb(206, 210, 218) };
        painter.rect_filled(r, egui::CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, fill);
        if i % 7 == 0 {
            painter.text(
                pos2(r.center().x, r.bottom() - 7.0),
                Align2::CENTER_CENTER,
                format!("C{}", i / 7 + 1),
                FontId::proportional(8.0),
                Color32::from_rgb(96, 102, 116),
            );
        }
    }
    for i in 0..WHITE_KEYS - 1 {
        if !has_black(i) {
            continue;
        }
        let note = white_note(i) + 1;
        let x = rect.left() + (i + 1) as f32 * kw;
        let r = Rect::from_min_max(pos2(x - black_w * 0.5, rect.top()), pos2(x + black_w * 0.5, rect.top() + black_h));
        let fill = if lit(note) { color.gamma_multiply(0.75) } else { Color32::from_rgb(22, 24, 29) };
        painter.rect_filled(r, egui::CornerRadius { nw: 0, ne: 0, sw: 2, se: 2 }, fill);
    }
    let hint = if is_standalone() {
        "Click keys to play (lower on the key = louder).\nComputer keyboard: A W S E D F T G Y H U J K = notes, Z / X = octave down / up."
    } else {
        "Click keys to audition the sound (lower on the key = louder). Notes from your DAW or keyboard light up here."
    };
    resp.on_hover_text(hint);
}

/// A row of evenly spaced knob centres: `at(i)` is the centre of knob `i`.
#[derive(Clone, Copy)]
pub struct Row {
    pub x: f32,
    pub y: f32,
    pub step: f32,
}

impl Row {
    pub fn new(x: f32, y: f32, step: f32) -> Self {
        Self { x, y, step }
    }

    pub fn at(&self, i: usize) -> Pos2 {
        pos2(self.x + self.step * i as f32, self.y)
    }
}
