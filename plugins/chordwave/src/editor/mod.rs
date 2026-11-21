//! egui editor for Chordwave.
//!
//! Layout (1200 x 860): header with key / scale / genre, the chord palette and the idea panel
//! (next-chord suggestions, melody matching, progression templates), the sequence of chords, a
//! piano roll for the melody with the harmony drawn underneath it, and panels for styles,
//! playback routing and MIDI export / import.

mod bottom;
mod ideas;
mod palette;
mod roll;
mod sequence;
pub mod widgets;

use crate::midifile::MidiIn;
use crate::params::*;
use crate::project::*;
use crate::render::RenderCfg;
use crate::style::*;
use crate::theory::*;
use crate::Shared;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::egui::{self, pos2, vec2, Align2, Color32, FontId, Id, Rect, Response, Sense, StrokeKind, Ui};
use plugin_common::ui::*;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use widgets::*;

/// Which chord set the palette shows.
#[derive(Clone, Copy, PartialEq)]
pub enum Source {
    Home,
    /// Steps along the circle of fifths from the home key.
    Fifths(i32),
    /// A parallel mode (index into `SCALES`) on the home tonic.
    Mode(usize),
}

#[derive(Clone, Copy, PartialEq)]
pub enum RightTab {
    Next,
    Match,
    Progressions,
}

pub struct State {
    pub sel: Option<Sel>,
    /// The section new chords are added to.
    pub cur_section: usize,
    pub source: Source,
    pub tab: RightTab,
    pub seed: u32,
    /// Snap of the piano roll in beats.
    pub snap: usize,
    pub zoom: Option<f32>,
    pub roll_scroll: f32,
    pub roll: roll::RollState,
    pub drag: Option<Chord>,
    pub audition: Option<(Id, Vec<u8>)>,
    pub template: usize,
    pub list_scroll: [f32; 3],
    pub import: Option<MidiIn>,
    pub import_name: String,
    pub transpose_with_key: bool,
    pub message: Option<(String, f64)>,
    /// The melody before the last generate / clear / fix, for UNDO.
    pub undo_melody: Option<Vec<Note>>,
    /// Drums are exported as Neon Break pattern keys (true) or as plain drum notes (false).
    pub drum_keys: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            sel: None,
            cur_section: 0,
            source: Source::Home,
            tab: RightTab::Next,
            seed: 1,
            snap: 1,
            zoom: None,
            roll_scroll: 0.0,
            roll: Default::default(),
            drag: None,
            audition: None,
            template: 0,
            list_scroll: [0.0; 3],
            import: None,
            import_name: String::new(),
            transpose_with_key: true,
            message: None,
            undo_melody: None,
            drum_keys: true,
        }
    }
}

/// Everything a panel needs for one frame.
pub struct Cx<'a> {
    pub p: &'a ChordwaveParams,
    pub shared: &'a Arc<Shared>,
    pub setter: &'a ParamSetter<'a>,
    pub key: u8,
    pub scale: Scale,
    pub genre: Genre,
    pub flats: bool,
    pub project: Project,
    pub cfg: RenderCfg,
    pub time: f64,
}

pub const SNAPS: [(&str, f32); 3] = [("1/4", 1.0), ("1/8", 0.5), ("1/16", 0.25)];

pub fn set<P: Param>(setter: &ParamSetter, p: &P, value: P::Plain) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, value);
    setter.end_set_parameter(p);
}

impl<'a> Cx<'a> {
    pub fn chord_notes(&self, chord: &Chord) -> Vec<u8> {
        chord_notes(chord, 0, self.cfg.spread, self.cfg.center)
    }

    pub fn flash(&self, st: &mut State, text: impl Into<String>) {
        st.message = Some((text.into(), self.time));
    }

    /// Add a chord after the selected one (or at the end of the current section).
    pub fn add_chord(&self, st: &mut State, chord: Chord) {
        let sel = st.sel;
        let section = st.cur_section;
        let new_sel = self.p.project.edit(|pr| {
            if let Some(s) = sel.filter(|s| pr.item(*s).is_some()) {
                let beats = pr.item(s).map(|i| i.beats).unwrap_or(4.0);
                pr.insert_item(s.section, s.item + 1, ChordItem::new(chord, beats))
            } else {
                let si = section.min(pr.sections.len().saturating_sub(1));
                let beats = pr.sections.get(si).and_then(|s| s.items.last()).map(|i| i.beats).unwrap_or(4.0);
                let at = pr.sections.get(si).map_or(0, |s| s.items.len());
                pr.insert_item(si, at, ChordItem::new(chord, beats))
            }
        });
        st.cur_section = new_sel.section;
        st.sel = Some(new_sel);
    }

    pub fn start_audition(&self, st: &mut State, id: Id, chord: &Chord) {
        self.stop_audition(st);
        let notes = self.chord_notes(chord);
        for &n in &notes {
            self.shared.pads.push(0x90, n, 100);
        }
        st.audition = Some((id, notes));
    }

    pub fn stop_audition(&self, st: &mut State) {
        if let Some((_, notes)) = st.audition.take() {
            for n in notes {
                self.shared.pads.push(0x80, n, 0);
            }
        }
    }

    /// Hold a single note for the piano roll preview (part 2 = melody).
    pub fn note_on(&self, pitch: u8) {
        self.shared.pads.push(0x92, pitch, 100);
    }
    pub fn note_off(&self, pitch: u8) {
        self.shared.pads.push(0x82, pitch, 0);
    }
}

impl State {
    /// Press = listen while held; a drag from the chip starts moving the chord; returns true on double-click.
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

pub fn create(params: Arc<ChordwaveParams>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        params.editor_state.clone(),
        State::default(),
        |ctx, _| apply_style(ctx, ACCENT),
        move |ctx, setter, st| {
            ctx.request_repaint_after(Duration::from_millis(33));
            let p = &*params;
            egui::CentralPanel::default().frame(egui::Frame::new().fill(BG)).show(ctx, |ui| {
                let o = ui.max_rect().min.to_vec2();
                let r = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(pos2(x, y) + o, vec2(w, h));
                let key = p.key_pc();
                let scale = p.scale_v();
                let cx = Cx {
                    p,
                    shared: &shared,
                    setter,
                    key,
                    scale,
                    genre: p.genre_v(),
                    flats: use_flats(key, scale),
                    project: p.project.get(),
                    cfg: p.render_cfg(),
                    time: ui.input(|i| i.time),
                };
                if let Some(sel) = st.sel {
                    if cx.project.item(sel).is_none() {
                        st.sel = None;
                    }
                }
                st.cur_section = st.cur_section.min(cx.project.sections.len().saturating_sub(1));
                bottom::take_messages(&cx, st);

                header(ui, &cx, st, r(0.0, 0.0, 1200.0, 56.0));
                palette::draw(ui, &cx, st, r(12.0, 62.0, 640.0, 250.0));
                ideas::draw(ui, &cx, st, r(660.0, 62.0, 528.0, 250.0));
                let seq_rect = r(12.0, 318.0, 1176.0, 132.0);
                sequence::draw(ui, &cx, st, seq_rect);
                roll::draw(ui, &cx, st, r(12.0, 456.0, 1176.0, 244.0));
                bottom::draw(ui, &cx, st, r(12.0, 706.0, 1176.0, 146.0));

                sequence::finish_drag(ui, &cx, st, seq_rect);
            });
        },
    )
}

fn header(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    let painter = ui.painter();
    painter.text(pos2(l + 18.0, t + 25.0), Align2::LEFT_CENTER, "CHORDWAVE", FontId::proportional(25.0), TEXT);
    spaced_text(painter, pos2(l + 19.0, t + 46.0), "CHORD & MELODY SKETCHER", 8.5, ACCENT, 1.6);

    // Key, scale and genre.
    caption(ui, pos2(l + 262.0, t + 8.0), "KEY");
    caption(ui, pos2(l + 336.0, t + 8.0), "SCALE");
    caption(ui, pos2(l + 506.0, t + 8.0), "GENRE");
    let key_names: Vec<String> = (0..12u8).map(|k| pc_name(k, use_flats(k, cx.scale)).to_string()).collect();
    let current_key = pc_name(cx.key, cx.flats);
    if let Some(i) = dd(ui, Rect::from_min_size(pos2(l + 254.0, t + 18.0), vec2(70.0, 26.0)), "key", current_key, &key_names, ACCENT,
        "The tonic of your song. Changing it moves the whole sketch when TRANSPOSE is on.") {
        if st.transpose_with_key {
            let diff = (i as i32 - cx.key as i32 + 18).rem_euclid(12) - 6;
            transpose(cx, diff);
        }
        set(cx.setter, &cx.p.key, i as i32);
    }
    let scale_names: Vec<&str> = SCALES.iter().map(|s| s.name).collect();
    let tip = format!("{}: {}", cx.scale.name(), cx.scale.def().blurb);
    if let Some(i) = dd(ui, Rect::from_min_size(pos2(l + 330.0, t + 18.0), vec2(160.0, 26.0)), "scale", cx.scale.name(), &scale_names, ACCENT, &tip) {
        set(cx.setter, &cx.p.scale, i as i32);
    }
    let genre_names: Vec<&str> = GENRES.iter().map(|g| g.name()).collect();
    if let Some(i) = dd(ui, Rect::from_min_size(pos2(l + 498.0, t + 18.0), vec2(140.0, 26.0)), "genre", cx.genre.name(), &genre_names, ACCENT,
        "Biases suggestions, templates and generated parts. It never restricts what you can use.") {
        let g = Genre::from_index(i);
        set(cx.setter, &cx.p.genre, i as i32);
        let (cs, bs) = g.default_styles();
        set(cx.setter, &cx.p.chord_style, cs.index() as i32);
        set(cx.setter, &cx.p.bass_style, bs.index() as i32);
        set(cx.setter, &cx.p.bpm, g.tempo());
    }
    if pill(ui, Rect::from_min_size(pos2(l + 644.0, t + 22.0), vec2(74.0, 18.0)), "transpose", "TRANSPOSE", st.transpose_with_key, AQUA,
        "On: changing the key moves your chords and melody with it.\nOff: the notes stay, only the key changes (the palette re-labels them).") {
        st.transpose_with_key = !st.transpose_with_key;
    }

    // Tempo, transport, loop.
    knob(ui, cx.setter, &cx.p.bpm, pos2(l + 760.0, t + 27.0), Kn::S, "TEMPO", ACCENT,
        "Tempo of the standalone app and of PLAY when the host does not give one. In a DAW, the host tempo is used while it plays.");
    let playing = cx.shared.play.load(Ordering::Relaxed);
    if button(ui, Rect::from_min_size(pos2(l + 800.0, t + 16.0), vec2(64.0, 28.0)), "play", if playing { "STOP" } else { "PLAY" },
        if playing { ACCENT } else { AQUA }, "Listen to the sketch on the built-in clock. In a DAW it also follows the host transport.") {
        cx.shared.play.store(!playing, Ordering::Relaxed);
    }
    toggle(ui, cx.setter, &cx.p.repeat, Rect::from_min_size(pos2(l + 872.0, t + 22.0), vec2(48.0, 18.0)), "LOOP", AQUA,
        "Repeat the sketch endlessly. Off: play it once, then stop.");

    // Scale notes.
    // In scale order starting from the tonic (A B C D E F G), not in pitch-class order.
    let notes: Vec<&str> = cx.scale.intervals().iter().map(|i| pc_name((cx.key + i) % 12, cx.flats)).collect();
    spaced_text(ui.painter(), pos2(l + 940.0, t + 22.0), "SCALE NOTES", 8.0, FAINT, 1.2);
    ui.painter().text(pos2(l + 940.0, t + 38.0), Align2::LEFT_CENTER, notes.join("  "), FontId::proportional(11.0), AQUA);

    knob(ui, cx.setter, &cx.p.gain, pos2(l + 1112.0, t + 27.0), Kn::S, "VOLUME", ACCENT, "Output level of the built-in sound.");
    meter(ui, Rect::from_min_size(pos2(l + 1136.0, t + 21.0), vec2(56.0, 10.0)), cx.shared.meter.load(Ordering::Relaxed), ACCENT);
}

/// Move every chord and melody note by `semitones`.
fn transpose(cx: &Cx, semitones: i32) {
    if semitones == 0 {
        return;
    }
    cx.p.project.edit(|pr| {
        for s in &mut pr.sections {
            for it in &mut s.items {
                it.chord.root = (it.chord.root as i32 + semitones).rem_euclid(12) as u8;
                it.chord.bass = it.chord.bass.map(|b| (b as i32 + semitones).rem_euclid(12) as u8);
            }
        }
        for n in &mut pr.melody {
            n.pitch = (n.pitch as i32 + semitones).clamp(0, 127) as u8;
        }
    });
}

/// Draw a drag ghost under the pointer.
pub fn ghost(ui: &Ui, chord: &Chord, flats: bool) {
    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
        let rect = Rect::from_center_size(pos + vec2(14.0, 10.0), vec2(70.0, 26.0));
        let col = kind_color(chord.kind());
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("ghost")));
        painter.rect_filled(rect, 5.0, col.gamma_multiply(0.5));
        painter.rect_stroke(rect, 5.0, st(1.5, col), egui::StrokeKind::Inside);
        painter.text(rect.center(), Align2::CENTER_CENTER, chord.name(flats), FontId::proportional(12.5), Color32::WHITE);
    }
}
