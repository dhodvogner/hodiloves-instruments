//! Step 5: getting the sketch into a DAW (drag, save, import). Also the MIDI file writing shared by every step.

use super::*;
use crate::drums::{render_drums, DrumMode};
use crate::midifile::*;
use crate::render::render;
use crate::suggest::{detect_chords, detect_key, pc_weights};
use std::path::PathBuf;

/// Pick up messages from the file dialog threads.
pub fn take_messages(cx: &Cx, st: &mut State) {
    if let Some(m) = cx.shared.status.lock().ok().and_then(|mut s| s.take()) {
        cx.flash(st, m);
    }
    if let Some(imported) = cx.shared.imported.lock().ok().and_then(|mut s| s.take()) {
        st.import = Some(imported);
    }
}


#[derive(Clone, Copy, PartialEq)]
pub enum Which {
    All,
    Chords,
    Bass,
    Melody,
    Drums,
}

impl Which {
    fn suffix(self) -> &'static str {
        match self {
            Which::All => "all",
            Which::Chords => "chords",
            Which::Bass => "bass",
            Which::Melody => "melody",
            Which::Drums => "drums",
        }
    }
}

/// The MIDI file for one part (or all), or `None` when there is nothing to write.
pub fn midi_bytes(cx: &Cx, which: Which, drum_keys: bool) -> Option<Vec<u8>> {
    let r = render(&cx.project, &cx.cfg);
    let mut tracks = Vec::new();
    let mut add = |name: &str, channel: u8, notes: Vec<Note>| {
        if !notes.is_empty() {
            tracks.push(TrackOut { name: name.to_string(), channel, notes });
        }
    };
    if matches!(which, Which::All | Which::Chords) {
        add("Chords", 0, r.chords.clone());
    }
    if matches!(which, Which::All | Which::Bass) {
        add("Bass", 1, r.bass.clone());
    }
    if matches!(which, Which::All | Which::Melody) {
        add("Melody", 2, r.melody.clone());
    }
    if which == Which::Drums {
        let mode = if drum_keys { DrumMode::PatternKeys } else { DrumMode::Notes };
        add(if drum_keys { "Neon Break keys" } else { "Drums" }, if drum_keys { 0 } else { 9 }, render_drums(&cx.project, cx.genre, mode));
    }
    let mut markers = Vec::new();
    let mut t = 0.0;
    for s in &cx.project.sections {
        if !s.items.is_empty() {
            markers.push((t, s.name.clone()));
        }
        t += s.beats();
    }
    if tracks.is_empty() {
        return None;
    }
    Some(write_smf(&SongInfo { bpm: cx.p.bpm.value(), key: cx.key, scale: cx.scale, markers }, &tracks))
}

fn file_stem(cx: &Cx, which: Which, drum_keys: bool) -> String {
    let key = pc_name(cx.key, cx.flats).replace('#', "sharp");
    let part = match (which, drum_keys) {
        (Which::Drums, true) => "drums neon-break keys",
        _ => which.suffix(),
    };
    format!("Chordwave {key} {} {}bpm ({part})", cx.scale.name().replace(' ', ""), cx.p.bpm.value().round())
}

const NOTHING: &str = "Nothing to export yet: add some chords first.";

pub fn save_dialog(cx: &Cx, st: &mut State, which: Which) {
    let Some(bytes) = midi_bytes(cx, which, st.drum_keys) else {
        cx.flash(st, NOTHING);
        return;
    };
    let name = format!("{}.mid", file_stem(cx, which, st.drum_keys));
    let shared = cx.shared.clone();
    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new().add_filter("MIDI file", &["mid"]).set_file_name(&name).save_file() else { return };
        let msg = match std::fs::write(&path, bytes) {
            Ok(()) => format!("Saved {}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            Err(e) => format!("Could not save: {e}"),
        };
        if let Ok(mut s) = shared.status.lock() {
            *s = Some(msg);
        }
    });
}

fn export_folder() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    match home {
        Some(h) if h.join("Documents").is_dir() => h.join("Documents").join("Chordwave"),
        Some(h) => h.join("Chordwave"),
        None => std::env::temp_dir().join("Chordwave"),
    }
}

fn open_folder(path: &std::path::Path) {
    let program = if cfg!(target_os = "windows") { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let _ = std::process::Command::new(program).arg(path).spawn();
}

/// Write the all-in-one file, one file per part and both drum versions into a folder, then show it.
pub fn quick_export(cx: &Cx, st: &mut State) {
    let dir = export_folder();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        cx.flash(st, format!("Could not create {}: {e}", dir.display()));
        return;
    }
    let mut written = 0;
    for (which, keys) in [(Which::All, true), (Which::Chords, true), (Which::Bass, true), (Which::Melody, true), (Which::Drums, true), (Which::Drums, false)] {
        if let Some(bytes) = midi_bytes(cx, which, keys) {
            if std::fs::write(dir.join(format!("{}.mid", file_stem(cx, which, keys))), bytes).is_ok() {
                written += 1;
            }
        }
    }
    if written == 0 {
        cx.flash(st, NOTHING);
        return;
    }
    open_folder(&dir);
    cx.flash(st, format!("Wrote {written} files to {}", dir.display()));
}

/// Write one part to a temporary file and drag it out of the window into the host.
pub fn drag_out(cx: &Cx, st: &mut State, which: Which) {
    let Some(bytes) = midi_bytes(cx, which, st.drum_keys) else {
        cx.flash(st, NOTHING);
        return;
    };
    let dir = std::env::temp_dir().join("Chordwave");
    let path = dir.join(format!("{}.mid", file_stem(cx, which, st.drum_keys)));
    if std::fs::create_dir_all(&dir).is_err() || std::fs::write(&path, bytes).is_err() {
        cx.flash(st, "Could not write the temporary MIDI file.");
        return;
    }
    if crate::dragout::drag_file(&path) {
        cx.flash(st, format!("Dragged {}. If nothing arrived, use a click to save the file.", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()));
    } else {
        // No window drag on this platform: save it instead.
        save_dialog(cx, st, which);
    }
}

pub fn import_dialog(cx: &Cx) {
    let shared = cx.shared.clone();
    plugin_common::preset_io::pick_file("MIDI file", &["mid", "midi"], move |path| {
        let result = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| read_smf(&b));
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match result {
            Ok(midi) => {
                if let Ok(mut s) = shared.imported.lock() {
                    *s = Some(midi);
                }
                if let Ok(mut s) = shared.status.lock() {
                    *s = Some(format!("Imported {name}"));
                }
            }
            Err(e) => {
                if let Ok(mut s) = shared.status.lock() {
                    *s = Some(format!("Could not import {name}: {e}"));
                }
            }
        }
    });
}


pub fn import_as_melody(cx: &Cx, st: &mut State, track: &TrackIn, bpm: Option<f32>) {
    st.undo_melody = Some(cx.project.melody.clone());
    let notes: Vec<Note> = track.notes.iter().map(|n| Note::new(n.start, n.len.max(0.0625), n.pitch, n.vel)).collect();
    let end = notes.iter().map(Note::end).fold(0.0, f32::max);
    // If the sketch is shorter than the melody, detect chords for the missing part.
    let need_chords = cx.project.beats() + 0.01 < end;
    let chords = need_chords.then(|| detect_chords(&notes, cx.key, cx.scale, cx.genre, BEATS_PER_BAR));
    cx.p.project.edit(|pr| {
        if let Some(chords) = &chords {
            pr.sections = vec![Section { name: "A".into(), items: chords.iter().map(|(c, b)| ChordItem::new(*c, *b)).collect() }];
        }
        pr.melody = notes.clone();
        pr.clamp_melody();
    });
    if let Some(b) = bpm {
        set(cx.setter, &cx.p.bpm, b.clamp(50.0, 220.0));
    }
    cx.flash(st, format!("Melody set from \"{}\". Check the key, then try MATCH MELODY for other chord ideas.", track.name));
}

pub fn find_chords(cx: &Cx, st: &mut State, track: &TrackIn) {
    let chords = detect_chords(&track.notes, cx.key, cx.scale, cx.genre, BEATS_PER_BAR);
    if chords.is_empty() {
        cx.flash(st, "No chords found: try another track or set the key first.");
        return;
    }
    let count = chords.len();
    cx.p.project.edit(|pr| {
        pr.sections = vec![Section { name: "A".into(), items: chords.iter().map(|(c, b)| ChordItem::new(*c, *b)).collect() }];
        pr.clamp_melody();
    });
    st.sel = None;
    st.cur_section = 0;
    cx.flash(st, format!("Found {count} chord{} in \"{}\".", if count == 1 { "" } else { "s" }, track.name));
}

// ---------------------------------------------------------------------------------------------
// The step
// ---------------------------------------------------------------------------------------------

struct PadInfo {
    which: Which,
    title: &'static str,
    target: &'static str,
    what: &'static str,
    color: Color32,
}

const PADS: [PadInfo; 5] = [
    PadInfo { which: Which::Chords, title: "Chords", target: "for Remedy", what: "Your chord progression as notes, played in the chord style you chose.", color: MAJOR_C },
    PadInfo { which: Which::Bass, title: "Bass", target: "for Wub Machine", what: "A bass line that follows the chords: one note at a time.", color: MINOR_C },
    PadInfo { which: Which::Drums, title: "Drums", target: "for Neon Break", what: "A drum pattern with a fill at the end of every section.", color: AMBER },
    PadInfo { which: Which::Melody, title: "Melody", target: "for any synth", what: "The melody you drew or generated.", color: AQUA },
    PadInfo { which: Which::All, title: "Everything", target: "one file", what: "All parts as separate tracks, with tempo, key and section markers.", color: ACCENT },
];

fn pad(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect, info: &PadInfo) {
    let resp = ui.interact(rect, Id::new(("pad", info.title)), Sense::click_and_drag());
    let hot = resp.hovered() || resp.dragged();
    let painter = ui.painter();
    painter.rect_filled(rect, 14.0, if hot { info.color.gamma_multiply(0.26) } else { info.color.gamma_multiply(0.13) });
    painter.rect_stroke(rect, 14.0, st_stroke(if hot { 2.0 } else { 1.0 }, info.color.gamma_multiply(if hot { 1.0 } else { 0.55 })), StrokeKind::Inside);
    // A grip, so it reads as something you can pick up.
    for r in 0..3 {
        for c in 0..2 {
            painter.circle_filled(pos2(rect.center().x - 5.0 + c as f32 * 10.0, rect.top() + 24.0 + r as f32 * 8.0), 2.0, info.color.gamma_multiply(0.9));
        }
    }
    painter.text(pos2(rect.center().x, rect.top() + 66.0), Align2::CENTER_CENTER, info.title, FontId::proportional(20.0), TEXT);
    painter.text(pos2(rect.center().x, rect.top() + 88.0), Align2::CENTER_CENTER, info.target, FontId::proportional(12.5), info.color);
    wrapped(ui, Rect::from_min_size(pos2(rect.left() + 14.0, rect.top() + 110.0), vec2(rect.width() - 28.0, 60.0)), info.what, 11.5, DIM);
    painter.text(pos2(rect.center().x, rect.bottom() - 16.0), Align2::CENTER_CENTER, if hot { "drag me into FL Studio" } else { "drag  ·  or click to save" }, FontId::proportional(11.0), if hot { TEXT } else { FAINT });
    if resp.drag_started() {
        drag_out(cx, st, info.which);
    } else if resp.clicked() {
        save_dialog(cx, st, info.which);
    }
}

fn st_stroke(w: f32, c: Color32) -> egui::Stroke {
    plugin_common::ui::st(w, c)
}

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, area: Rect) {
    label(ui, area.left_top() + vec2(0.0, 14.0), Align2::LEFT_CENTER, "Take it to your DAW", 26.0, TEXT);
    label(ui, area.left_top() + vec2(0.0, 46.0), Align2::LEFT_CENTER, "Pick up a block and drop it on the piano roll of the instrument that should play it.", 13.5, DIM);

    let top = area.top() + 84.0;
    let gap = 14.0;
    let w = (area.width() - gap * 4.0) / 5.0;
    for (i, info) in PADS.iter().enumerate() {
        pad(ui, cx, st, Rect::from_min_size(pos2(area.left() + i as f32 * (w + gap), top), vec2(w, 196.0)), info);
    }

    // Instructions and the other ways out.
    let y = top + 220.0;
    let left = Rect::from_min_size(pos2(area.left(), y), vec2(area.width() * 0.56 - 10.0, 190.0));
    card(ui, left);
    caps(ui, left.left_top() + vec2(20.0, 22.0), "IN FL STUDIO");
    let steps = [
        "1.  Open the piano roll of the instrument (for example Remedy for the chords).",
        "2.  Drag the block here onto the piano roll window.",
        "3.  The notes land at the right place. Press play.",
    ];
    for (i, s) in steps.iter().enumerate() {
        label(ui, left.left_top() + vec2(20.0, 54.0 + i as f32 * 28.0), Align2::LEFT_CENTER, *s, 13.0, Color32::from_rgb(205, 210, 220));
    }
    wrapped(ui, Rect::from_min_size(left.left_top() + vec2(20.0, 142.0), vec2(left.width() - 40.0, 40.0)),
        "If dragging does nothing in your setup, use \"Save all to a folder\" and drag the files from there. Dragging from this window works on Windows.", 11.5, FAINT);

    let right = Rect::from_min_size(pos2(left.right() + 20.0, y), vec2(area.right() - left.right() - 20.0, 190.0));
    card(ui, right);
    caps(ui, right.left_top() + vec2(20.0, 22.0), "OTHER OPTIONS");
    let bw = right.width() - 40.0;
    if big_button(ui, Rect::from_min_size(right.left_top() + vec2(20.0, 44.0), vec2(bw, 40.0)), "quick", "Save all to a folder", Btn::Normal,
        "Writes every file into Documents/Chordwave and opens the folder.") {
        quick_export(cx, st);
    }
    if big_button(ui, Rect::from_min_size(right.left_top() + vec2(20.0, 92.0), vec2(bw, 40.0)), "import", "Import a MIDI file…", Btn::Normal,
        "Load a melody, bass line or chords from a MIDI file and build around it.") {
        import_dialog(cx);
    }
    let label_keys = if st.drum_keys { "Drums: Neon Break patterns" } else { "Drums: plain drum notes" };
    if big_button(ui, Rect::from_min_size(right.left_top() + vec2(20.0, 140.0), vec2(bw, 34.0)), "drum-mode", label_keys, Btn::Normal,
        "Neon Break patterns: notes that trigger the pattern slots of Neon Break (keep its pattern mode on 'Pattern keys').\nPlain drum notes: kick, snare and hats that any drum machine understands.") {
        st.drum_keys = !st.drum_keys;
    }

    let msg = st.message.clone().filter(|(_, at)| cx.time - at < 8.0);
    if let Some((text, _)) = msg {
        let r = Rect::from_min_size(pos2(area.left(), area.bottom() - 40.0), vec2(area.width(), 32.0));
        ui.painter().rect_filled(r, 8.0, AQUA.gamma_multiply(0.14));
        wrapped(ui, r.shrink2(vec2(14.0, 8.0)), &text, 12.0, AQUA);
    }
}

/// The track chooser shown after a MIDI file was picked (on top of whatever step is open).
pub fn import_overlay(ui: &mut Ui, cx: &Cx, st: &mut State, win: Rect) {
    let Some(midi) = st.import.clone() else { return };
    ui.painter().rect_filled(win, 0.0, Color32::from_black_alpha(170));
    let r = Rect::from_center_size(win.center(), vec2(640.0, 360.0));
    ui.painter().rect_filled(r, 16.0, Color32::from_rgb(32, 36, 44));
    ui.painter().rect_stroke(r, 16.0, st_stroke(1.0, LINE), StrokeKind::Inside);
    let l = r.left() + 28.0;
    let t = r.top() + 28.0;
    label(ui, pos2(l, t + 6.0), Align2::LEFT_CENTER, "What is in this file?", 22.0, TEXT);
    let all: Vec<Note> = midi.tracks.iter().filter(|tr| !tr.drums).flat_map(|tr| tr.notes.clone()).collect();
    let guess = detect_key(&pc_weights(&all)).first().copied();
    let mut sub = String::new();
    if let Some(b) = midi.bpm {
        sub.push_str(&format!("{:.0} BPM  ·  ", b));
    }
    if let Some(g) = guess {
        sub.push_str(&format!("sounds like {} {}", pc_name(g.key, use_flats(g.key, g.scale)), g.scale.name()));
    }
    label(ui, pos2(l, t + 36.0), Align2::LEFT_CENTER, sub, 13.0, AMBER);
    if let Some(g) = guess {
        if g.key != cx.key || g.scale != cx.scale {
            if big_button(ui, Rect::from_min_size(pos2(r.right() - 188.0, t - 4.0), vec2(160.0, 34.0)), "imp-key", "Use this key", Btn::Normal, "Set the key and scale to what the file sounds like.") {
                set(cx.setter, &cx.p.key, g.key as i32);
                set(cx.setter, &cx.p.scale, g.scale.index() as i32);
            }
        }
    }
    let tracks: Vec<&TrackIn> = midi.tracks.iter().filter(|tr| !tr.drums).take(4).collect();
    for (i, track) in tracks.iter().enumerate() {
        let y = t + 72.0 + i as f32 * 52.0;
        let row = Rect::from_min_size(pos2(l, y), vec2(r.width() - 56.0, 44.0));
        ui.painter().rect_filled(row, 8.0, RAISED);
        label(ui, row.left_center() + vec2(14.0, 0.0), Align2::LEFT_CENTER, format!("{}   ({} notes)", ellipsize(&track.name, 28), track.notes.len()), 13.0, TEXT);
        if big_button(ui, Rect::from_min_size(row.right_top() + vec2(-270.0, 5.0), vec2(124.0, 34.0)), &format!("imp-mel{i}"), "Use as melody", Btn::Normal,
            "Use this track as the melody (chords are detected for it if the sketch is shorter).") {
            import_as_melody(cx, st, track, midi.bpm);
            st.import = None;
            return;
        }
        if big_button(ui, Rect::from_min_size(row.right_top() + vec2(-140.0, 5.0), vec2(130.0, 34.0)), &format!("imp-chd{i}"), "Find the chords", Btn::Primary,
            "Detect a chord for every bar of this track and replace the progression.") {
            find_chords(cx, st, track);
            st.import = None;
            return;
        }
    }
    if tracks.is_empty() {
        label(ui, pos2(l, t + 90.0), Align2::LEFT_CENTER, "Only drum tracks found.", 13.0, DIM);
    }
    if big_button(ui, Rect::from_min_size(pos2(r.right() - 128.0, r.bottom() - 54.0), vec2(100.0, 36.0)), "imp-close", "Cancel", Btn::Quiet, "") {
        st.import = None;
    }
}
