//! Small drawing helpers shared by the editor panels.

use crate::theory::*;
use nih_plug_egui::egui::{self, pos2, vec2, Align2, Color32, FontId, Id, Rect, Response, Sense, Shape, StrokeKind, Ui};
use plugin_common::ui::*;

/// Accent of the whole plugin: neon pink.
pub const ACCENT: Color32 = Color32::from_rgb(255, 84, 160);
pub const AQUA: Color32 = Color32::from_rgb(84, 226, 206);
pub const MAJOR_C: Color32 = Color32::from_rgb(96, 222, 164);
pub const MINOR_C: Color32 = Color32::from_rgb(182, 128, 255);
pub const OTHER_C: Color32 = Color32::from_rgb(150, 158, 176);
pub const AMBER: Color32 = Color32::from_rgb(255, 176, 64);

pub fn kind_color(kind: Kind) -> Color32 {
    match kind {
        Kind::Major => MAJOR_C,
        Kind::Minor => MINOR_C,
        Kind::Other => OTHER_C,
    }
}

pub fn function_color(f: Function) -> Color32 {
    match f {
        Function::Tonic => AQUA,
        Function::Predominant => AMBER,
        Function::Dominant => ACCENT,
        Function::Colour => MINOR_C,
    }
}

fn chevron(painter: &egui::Painter, c: egui::Pos2, color: Color32) {
    painter.add(Shape::line(vec![c + vec2(-3.5, -1.5), c + vec2(0.0, 2.0), c + vec2(3.5, -1.5)], st(1.4, color)));
}

/// A pill-shaped switch that is not a parameter. Returns true when clicked.
pub fn pill(ui: &mut Ui, rect: Rect, id: &str, label: &str, on: bool, color: Color32, tip: &str) -> bool {
    let resp = ui.interact(rect, Id::new(("pill", id)), Sense::click());
    let painter = ui.painter();
    let radius = rect.height() * 0.5;
    if on {
        painter.rect_filled(rect, radius, color.gamma_multiply(0.18));
        painter.rect_stroke(rect, radius, st(1.0, color.gamma_multiply(0.8)), StrokeKind::Inside);
    } else {
        painter.rect_filled(rect, radius, if resp.hovered() { RAISED_HOVER } else { RAISED });
    }
    painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(9.5), if on { color } else { DIM });
    let clicked = resp.clicked();
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    clicked
}

/// A drop-down list of text options. Returns the index picked this frame.
pub fn dd<S: AsRef<str>>(ui: &mut Ui, rect: Rect, id: &str, current: &str, options: &[S], color: Color32, tip: &str) -> Option<usize> {
    let wid = Id::new(("dd", id));
    let popup_id = wid.with("popup");
    let resp = ui.interact(rect, wid, Sense::click());
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 5.0, if resp.hovered() { RAISED_HOVER } else { RAISED });
    let clip = Rect::from_min_max(rect.min, pos2(rect.right() - 16.0, rect.bottom()));
    ui.painter_at(clip).text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, current, FontId::proportional(11.0), color);
    chevron(painter, pos2(rect.right() - 10.0, rect.center().y), DIM);

    let mut pick = None;
    egui::popup_below_widget(ui, popup_id, &resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        ui.set_min_width(rect.width().max(120.0));
        egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
            for (i, o) in options.iter().enumerate() {
                if ui.selectable_label(o.as_ref() == current, egui::RichText::new(o.as_ref()).size(11.5)).clicked() {
                    pick = Some(i);
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        });
    });
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    pick
}

/// Text drawn inside a rectangle, wrapped to its width, clipped to its height.
pub fn wrapped(ui: &Ui, rect: Rect, text: &str, size: f32, color: Color32) {
    let painter = ui.painter_at(rect);
    let galley = painter.layout(text.to_string(), FontId::proportional(size), color, rect.width());
    painter.galley(rect.min, galley, color);
}

/// Truncate to what fits in `max_chars`, with an ellipsis.
pub fn ellipsize(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", cut.trim_end())
    }
}

/// A chord cell: a coloured rounded rectangle with the chord name, an optional roman numeral and a
/// filled (in key) or hollow (borrowed) dot. Returns the response (click + drag).
pub fn chord_cell(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    chord: &Chord,
    flats: bool,
    roman_text: Option<&str>,
    in_key: bool,
    selected: bool,
) -> Response {
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let col = kind_color(chord.kind());
    let painter = ui.painter();
    let fill = if resp.hovered() || resp.is_pointer_button_down_on() { col.gamma_multiply(0.34) } else { col.gamma_multiply(0.16) };
    painter.rect_filled(rect, 5.0, fill);
    if selected {
        painter.rect_stroke(rect, 5.0, st(1.5, ACCENT), StrokeKind::Inside);
    } else {
        painter.rect_stroke(rect, 5.0, st(1.0, col.gamma_multiply(0.45)), StrokeKind::Inside);
    }
    let c = pos2(rect.left() + 9.0, rect.top() + 9.0);
    if in_key {
        painter.circle_filled(c, 3.0, col);
    } else {
        painter.circle_stroke(c, 3.0, st(1.2, col));
    }
    let name_pos = if roman_text.is_some() { pos2(rect.center().x, rect.center().y - 3.5) } else { rect.center() };
    painter.text(name_pos, Align2::CENTER_CENTER, chord.name(flats), FontId::proportional(12.5), TEXT);
    if let Some(r) = roman_text {
        painter.text(pos2(rect.center().x, rect.bottom() - 7.0), Align2::CENTER_CENTER, r, FontId::proportional(8.5), DIM);
    }
    resp
}

/// Hover text describing a chord: notes, numeral, function.
pub fn chord_tip(resp: Response, chord: &Chord, key: u8, scale: Scale, flats: bool) -> Response {
    let notes: Vec<&str> = chord.pitch_classes().iter().map(|p| pc_name(*p, flats)).collect();
    let rn = roman(key, scale, chord);
    let f = function_of(key, scale, chord);
    let in_key = chord.in_key(key, scale);
    resp.on_hover_ui(|ui| {
        ui.set_max_width(260.0);
        ui.label(egui::RichText::new(format!("{}   ({})", chord.name(flats), rn)).size(13.0).color(TEXT));
        ui.label(egui::RichText::new(format!("Notes: {}", notes.join("  "))).size(11.0).color(DIM));
        ui.label(egui::RichText::new(format!("{}: {}", f.label(), f.explain())).size(11.0).color(function_color(f)));
        if !in_key {
            ui.label(egui::RichText::new("Hollow dot: at least one note is outside the home key.").size(10.5).color(FAINT));
        }
        ui.label(egui::RichText::new("Click: listen  ·  Double-click: add  ·  Drag onto the sequence").size(10.5).color(FAINT));
    })
}

// ---------------------------------------------------------------------------------------------
// Roomy building blocks for the guided layout
// ---------------------------------------------------------------------------------------------

/// Plain text at a position.
pub fn label(ui: &Ui, pos: egui::Pos2, align: Align2, text: impl ToString, size: f32, color: Color32) {
    ui.painter().text(pos, align, text.to_string(), FontId::proportional(size), color);
}

/// A small upper-case section label.
pub fn caps(ui: &Ui, pos: egui::Pos2, text: &str) {
    spaced_text(ui.painter(), pos, text, 10.5, DIM, 1.6);
}

/// A soft rounded surface.
pub fn card(ui: &Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 12.0, PANEL);
}

#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    /// The one thing to do next.
    Primary,
    Normal,
    /// Text only.
    Quiet,
}

pub fn big_button(ui: &mut Ui, rect: Rect, id: &str, text: &str, kind: Btn, tip: &str) -> bool {
    let resp = ui.interact(rect, Id::new(("big", id)), Sense::click());
    let hot = resp.hovered();
    let painter = ui.painter();
    let (fill, fg) = match kind {
        Btn::Primary => (if hot { ACCENT.gamma_multiply(1.0) } else { ACCENT.gamma_multiply(0.85) }, Color32::WHITE),
        Btn::Normal => (if hot { RAISED_HOVER } else { RAISED }, TEXT),
        Btn::Quiet => (if hot { RAISED } else { Color32::TRANSPARENT }, if hot { TEXT } else { DIM }),
    };
    painter.rect_filled(rect, 10.0, fill);
    painter.text(rect.center(), Align2::CENTER_CENTER, text, FontId::proportional(14.0), fg);
    let clicked = resp.clicked();
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    clicked
}

/// A selectable card with a title and a short description. Returns true when clicked.
pub fn choice(ui: &mut Ui, rect: Rect, id: &str, title: &str, sub: &str, selected: bool, color: Color32, tip: &str) -> bool {
    let resp = ui.interact(rect, Id::new(("choice", id)), Sense::click());
    let hot = resp.hovered();
    let painter = ui.painter();
    painter.rect_filled(rect, 10.0, if selected { color.gamma_multiply(0.2) } else if hot { RAISED_HOVER } else { RAISED });
    painter.rect_stroke(rect, 10.0, st(if selected { 2.0 } else { 1.0 }, if selected { color } else { LINE }), StrokeKind::Inside);
    let pad = 14.0;
    if sub.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER, title, FontId::proportional(15.0), if selected { TEXT } else { Color32::from_rgb(200, 205, 215) });
    } else {
        painter.text(pos2(rect.left() + pad, rect.top() + rect.height() * 0.38), Align2::LEFT_CENTER, title, FontId::proportional(15.0), TEXT);
        let clip = ui.painter_at(rect.shrink2(vec2(pad - 4.0, 0.0)));
        clip.text(pos2(rect.left() + pad, rect.top() + rect.height() * 0.72), Align2::LEFT_CENTER, sub, FontId::proportional(11.5), DIM);
    }
    let clicked = resp.clicked();
    if !tip.is_empty() {
        resp.on_hover_text(tip);
    }
    clicked
}

/// A row of mutually exclusive options. Returns the index clicked.
pub fn segmented(ui: &mut Ui, rect: Rect, id: &str, options: &[&str], selected: usize, color: Color32) -> Option<usize> {
    let n = options.len().max(1) as f32;
    let w = rect.width() / n;
    ui.painter().rect_filled(rect, 10.0, INSET);
    let mut picked = None;
    for (i, name) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * w, rect.top()), vec2(w, rect.height())).shrink(3.0);
        let resp = ui.interact(r, Id::new(("seg", id, i)), Sense::click());
        let on = i == selected;
        if on {
            ui.painter().rect_filled(r, 8.0, color.gamma_multiply(0.9));
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 8.0, RAISED);
        }
        ui.painter().text(r.center(), Align2::CENTER_CENTER, *name, FontId::proportional(13.0), if on { Color32::WHITE } else { DIM });
        if resp.clicked() {
            picked = Some(i);
        }
    }
    picked
}

/// First sentence-ish part of a suggestion reason, in plain words.
pub fn short_reason(reason: &str) -> String {
    let parts: Vec<&str> = reason.split(';').map(str::trim).collect();
    let mut out = parts.iter().take(2).copied().collect::<Vec<_>>().join(". ");
    if let Some(c) = out.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    out
}

/// A vertical list of radio options. Returns (index clicked, index hovered).
pub fn radio_list(ui: &mut Ui, rect: Rect, id: &str, options: &[&str], selected: usize, color: Color32) -> (Option<usize>, Option<usize>) {
    let row_h = rect.height() / options.len().max(1) as f32;
    let mut picked = None;
    let mut hovered = None;
    for (i, name) in options.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left(), rect.top() + i as f32 * row_h), vec2(rect.width(), row_h)).shrink2(vec2(0.0, 1.5));
        let resp = ui.interact(r, Id::new(("radio", id, i)), Sense::click());
        if resp.hovered() {
            hovered = Some(i);
        }
        let on = i == selected;
        if on {
            ui.painter().rect_filled(r, 8.0, color.gamma_multiply(0.18));
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 8.0, RAISED);
        }
        let c = pos2(r.left() + 18.0, r.center().y);
        ui.painter().circle_stroke(c, 7.0, st(1.5, if on { color } else { FAINT }));
        if on {
            ui.painter().circle_filled(c, 3.8, color);
        }
        ui.painter().text(pos2(r.left() + 36.0, r.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(13.5), if on { TEXT } else { Color32::from_rgb(190, 196, 208) });
        if resp.clicked() {
            picked = Some(i);
        }
    }
    (picked, hovered)
}

/// An on / off switch row bound to a plain bool (returns the new value when clicked).
pub fn switch(ui: &mut Ui, rect: Rect, id: &str, text: &str, on: bool, color: Color32, tip: &str) -> Option<bool> {
    let resp = ui.interact(rect, Id::new(("switch", id)), Sense::click());
    let track = Rect::from_min_size(pos2(rect.left(), rect.center().y - 10.0), vec2(40.0, 20.0));
    let painter = ui.painter();
    painter.rect_filled(track, 10.0, if on { color } else { TRACK });
    painter.circle_filled(pos2(if on { track.right() - 10.0 } else { track.left() + 10.0 }, track.center().y), 7.5, Color32::WHITE);
    painter.text(pos2(track.right() + 12.0, rect.center().y), Align2::LEFT_CENTER, text, FontId::proportional(13.0), if on { TEXT } else { DIM });
    if !tip.is_empty() {
        resp.clone().on_hover_text(tip);
    }
    resp.clicked().then_some(!on)
}
