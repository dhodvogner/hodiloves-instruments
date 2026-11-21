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
