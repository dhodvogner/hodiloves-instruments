//! egui editor for Chordwave: a guided, five-step flow.
//!
//! 1 Start (genre, key, mood, a first progression) -> 2 Chords -> 3 Melody -> 4 Sound -> 5 Export.
//! One job per screen, big type, plain words. The header shows where you are and always has Play.

mod chords;
mod export;
mod melody;
mod sound;
mod start;
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Start,
    Chords,
    Melody,
    Sound,
    Export,
}

const STEPS: [(Step, &str); 5] = [(Step::Start, "Start"), (Step::Chords, "Chords"), (Step::Melody, "Melody"), (Step::Sound, "Sound"), (Step::Export, "Export")];

/// Which chord set the chord picker shows.
#[derive(Clone, Copy, PartialEq)]
pub enum Source {
    Home,
    /// Steps along the circle of fifths from the home key.
    Fifths(i32),
    /// A parallel mode (index into `SCALES`) on the home tonic.
    Mode(usize),
}

pub struct State {
    pub step: Step,
    pub sel: Option<Sel>,
    /// The section new chords are added to.
    pub cur_section: usize,
    pub source: Source,
    /// How rich the chords in the picker are.
    pub richness: Ext,
    pub seed: u32,
    /// Snap of the piano roll in beats.
    pub snap: usize,
    pub zoom: Option<f32>,
    pub roll_x: f32,
    pub roll_y: f32,
    pub roll_y_init: bool,
    /// Show every semitone in the piano roll instead of only the notes of the key.
    pub show_all: bool,
    pub roll: melody::RollState,
    pub drag: Option<Chord>,
    pub audition: Option<(Id, Vec<u8>)>,
    pub import: Option<MidiIn>,
    pub message: Option<(String, f64)>,
    /// The melody before the last generate / clear / fix, for UNDO.
    pub undo_melody: Option<Vec<Note>>,
    /// Drums are exported as Neon Break pattern keys (true) or as plain drum notes (false).
    pub drum_keys: bool,
    /// How the melody generator writes (see `generate::MelodyStyle`).
    pub melody_style: crate::generate::MelodyStyle,
    /// Short-lived animations in the piano roll (a note was added or removed).
    pub fx: Vec<melody::Fx>,
    /// Which part the Sound step is showing: 0 chords, 1 bass, 2 drums, 3 melody.
    pub sound_part: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            step: Step::Start,
            sel: None,
            cur_section: 0,
            source: Source::Home,
            richness: Ext::Triad,
            seed: 1,
            snap: 1,
            zoom: None,
            roll_x: 0.0,
            roll_y: 0.0,
            roll_y_init: false,
            show_all: false,
            roll: Default::default(),
            drag: None,
            audition: None,
            import: None,
            message: None,
            undo_melody: None,
            drum_keys: true,
            melody_style: crate::generate::MelodyStyle::Hook,
            fx: Vec::new(),
            sound_part: 0,
        }
    }
}

/// Everything a step needs for one frame.
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

pub const SNAPS: [(&str, f32); 3] = [("1/4 beat", 1.0), ("1/8 beat", 0.5), ("1/16 beat", 0.25)];

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

    /// Hold a single note for the piano roll preview (channel 3 = melody).
    pub fn note_on(&self, pitch: u8) {
        self.shared.pads.push(0x92, pitch, 100);
    }
    pub fn note_off(&self, pitch: u8) {
        self.shared.pads.push(0x82, pitch, 0);
    }

    /// Choose a genre: it also sets the chord and bass style and the tempo that suit it.
    pub fn apply_genre(&self, index: usize) {
        let g = Genre::from_index(index);
        set(self.setter, &self.p.genre, index as i32);
        let (cs, bs) = g.default_styles();
        set(self.setter, &self.p.chord_style, cs.index() as i32);
        set(self.setter, &self.p.bass_style, bs.index() as i32);
        set(self.setter, &self.p.bpm, g.tempo());
    }

    /// Change the key; the whole sketch moves with it.
    pub fn change_key(&self, new_key: u8) {
        let diff = (new_key as i32 - self.key as i32 + 18).rem_euclid(12) - 6;
        if diff != 0 {
            self.p.project.edit(|pr| {
                for s in &mut pr.sections {
                    for it in &mut s.items {
                        it.chord.root = (it.chord.root as i32 + diff).rem_euclid(12) as u8;
                        it.chord.bass = it.chord.bass.map(|b| (b as i32 + diff).rem_euclid(12) as u8);
                    }
                }
                for n in &mut pr.melody {
                    n.pitch = (n.pitch as i32 + diff).clamp(0, 127) as u8;
                }
            });
        }
        set(self.setter, &self.p.key, new_key as i32);
    }

    /// Replace the progression with a template, written in the current key.
    pub fn apply_template(&self, st: &mut State, tpl: &Template) {
        if tpl.scale != self.scale {
            set(self.setter, &self.p.scale, tpl.scale.index() as i32);
        }
        let key = self.key;
        self.p.project.edit(|pr| pr.sections = vec![tpl.section(key, "A")]);
        st.cur_section = 0;
        st.sel = None;
    }
}

const W: f32 = 1120.0;
const MARGIN: f32 = 32.0;

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
                export::take_messages(&cx, st);

                header(ui, &cx, st, r(0.0, 0.0, W, 72.0));
                let area = r(MARGIN, 88.0, W - 2.0 * MARGIN, 624.0);
                match st.step {
                    Step::Start => start::draw(ui, &cx, st, area),
                    Step::Chords => chords::draw(ui, &cx, st, area),
                    Step::Melody => melody::draw(ui, &cx, st, area),
                    Step::Sound => sound::draw(ui, &cx, st, area),
                    Step::Export => export::draw(ui, &cx, st, area),
                }
                footer(ui, st, r(MARGIN, 728.0, W - 2.0 * MARGIN, 48.0));
                if st.step == Step::Chords {
                    chords::finish_drag(ui, &cx, st);
                }
                export::import_overlay(ui, &cx, st, r(0.0, 0.0, W, 800.0));
            });
        },
    )
}

fn header(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let (l, t) = (rect.left(), rect.top());
    ui.painter().text(pos2(l + MARGIN, t + 36.0), Align2::LEFT_CENTER, "Chordwave", FontId::proportional(22.0), TEXT);
    ui.painter().circle_filled(pos2(l + MARGIN + 112.0, t + 33.0), 3.5, ACCENT);

    // The five steps.
    let pill_w = 116.0;
    let gap = 6.0;
    let total = 5.0 * pill_w + 4.0 * gap;
    let x0 = l + 200.0;
    let _ = total;
    for (i, (step, name)) in STEPS.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x0 + i as f32 * (pill_w + gap), t + 18.0), vec2(pill_w, 36.0));
        let resp = ui.interact(r, Id::new(("step", i)), Sense::click());
        let on = st.step == *step;
        let painter = ui.painter();
        painter.rect_filled(r, 18.0, if on { ACCENT } else if resp.hovered() { RAISED_HOVER } else { RAISED });
        let c = pos2(r.left() + 20.0, r.center().y);
        painter.circle_filled(c, 10.0, if on { Color32::from_black_alpha(60) } else { BG });
        painter.text(c, Align2::CENTER_CENTER, format!("{}", i + 1), FontId::proportional(12.0), if on { Color32::WHITE } else { DIM });
        painter.text(pos2(r.left() + 38.0, r.center().y), Align2::LEFT_CENTER, *name, FontId::proportional(14.0), if on { Color32::WHITE } else { Color32::from_rgb(200, 205, 215) });
        if resp.clicked() {
            cx.stop_audition(st);
            st.step = *step;
        }
    }

    // Tempo and transport, always within reach.
    let right = rect.right() - MARGIN;
    let playing = cx.shared.play.load(Ordering::Relaxed);
    if big_button(ui, Rect::from_min_size(pos2(right - 104.0, t + 16.0), vec2(104.0, 40.0)), "play", if playing { "Stop" } else { "Play" },
        if playing { Btn::Normal } else { Btn::Primary }, "Listen to your sketch. In a DAW it also follows the host's play button.") {
        cx.shared.play.store(!playing, Ordering::Relaxed);
    }
    let bpm = cx.p.bpm.value();
    let bx = right - 104.0 - 16.0 - 132.0;
    if big_button(ui, Rect::from_min_size(pos2(bx, t + 20.0), vec2(30.0, 32.0)), "bpm-dn", "−", Btn::Quiet, "Slower") {
        set(cx.setter, &cx.p.bpm, (bpm - 1.0).max(50.0));
    }
    label(ui, pos2(bx + 66.0, t + 36.0), Align2::CENTER_CENTER, format!("{:.0} BPM", bpm), 14.0, TEXT);
    if big_button(ui, Rect::from_min_size(pos2(bx + 102.0, t + 20.0), vec2(30.0, 32.0)), "bpm-up", "+", Btn::Quiet, "Faster") {
        set(cx.setter, &cx.p.bpm, (bpm + 1.0).min(220.0));
    }
    ui.painter().line_segment([pos2(l, t + 71.5), pos2(rect.right(), t + 71.5)], st_line());
}

fn st_line() -> egui::Stroke {
    plugin_common::ui::st(1.0, LINE)
}

fn footer(ui: &mut Ui, st: &mut State, rect: Rect) {
    let i = STEPS.iter().position(|(s, _)| *s == st.step).unwrap_or(0);
    if i > 0 && big_button(ui, Rect::from_min_size(rect.left_top() + vec2(0.0, 4.0), vec2(150.0, 42.0)), "back", &format!("Back: {}", STEPS[i - 1].1), Btn::Normal, "") {
        st.step = STEPS[i - 1].0;
    }
    if i + 1 < STEPS.len() && big_button(ui, Rect::from_min_size(rect.right_top() + vec2(-190.0, 4.0), vec2(190.0, 42.0)), "next", &format!("Next: {}", STEPS[i + 1].1), Btn::Primary, "") {
        st.step = STEPS[i + 1].0;
    }
    label(ui, pos2(rect.center().x, rect.center().y), Align2::CENTER_CENTER, format!("Step {} of {}", i + 1, STEPS.len()), 12.0, FAINT);
}

/// Draw a drag ghost under the pointer.
pub fn ghost(ui: &Ui, chord: &Chord, flats: bool) {
    if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
        let rect = Rect::from_center_size(pos + vec2(18.0, 12.0), vec2(84.0, 34.0));
        let col = kind_color(chord.kind());
        let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("ghost")));
        painter.rect_filled(rect, 8.0, col.gamma_multiply(0.55));
        painter.rect_stroke(rect, 8.0, st(1.5, col), StrokeKind::Inside);
        painter.text(rect.center(), Align2::CENTER_CENTER, chord.name(flats), FontId::proportional(15.0), Color32::WHITE);
    }
}
