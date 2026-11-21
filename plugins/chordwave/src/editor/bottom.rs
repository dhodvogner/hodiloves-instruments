//! Bottom row: styles and voicing, playback routing, and the bridge to your DAW (MIDI export / import).

use super::*;
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

pub fn draw(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    parts_panel(ui, cx, Rect::from_min_size(rect.min, vec2(420.0, rect.height())));
    playback_panel(ui, cx, Rect::from_min_size(rect.min + vec2(428.0, 0.0), vec2(330.0, rect.height())));
    export_panel(ui, cx, st, Rect::from_min_size(rect.min + vec2(766.0, 0.0), vec2(410.0, rect.height())));
}

fn param_dd(ui: &mut Ui, cx: &Cx, rect: Rect, id: &str, p: &IntParam, names: &[&str], tip: &str) {
    let cur = names[(p.value().max(0) as usize).min(names.len() - 1)];
    if let Some(i) = dd(ui, rect, id, cur, names, ACCENT, tip) {
        set(cx.setter, p, i as i32);
    }
}

fn parts_panel(ui: &mut Ui, cx: &Cx, rect: Rect) {
    let inner = panel(ui, rect, "PARTS & VOICING", ACCENT);
    let (l, t) = (inner.left(), inner.top());
    caption(ui, pos2(l + 2.0, t + 4.0), "CHORD PART");
    caption(ui, pos2(l + 206.0, t + 4.0), "BASS PART");
    let chord_names: Vec<&str> = CHORD_STYLES.iter().map(|s| s.name()).collect();
    let bass_names: Vec<&str> = BASS_STYLES.iter().map(|s| s.name()).collect();
    param_dd(ui, cx, Rect::from_min_size(pos2(l, t + 14.0), vec2(198.0, 24.0)), "cstyle", &cx.p.chord_style, &chord_names,
        "How each chord is played: held pads, pulses, gated 16ths, arpeggios... The same pattern is used for playback and for the MIDI export.");
    param_dd(ui, cx, Rect::from_min_size(pos2(l + 204.0, t + 14.0), vec2(196.0, 24.0)), "bstyle", &cx.p.bass_style, &bass_names,
        "A bass line that follows the chord roots (or the slash bass), in a register that suits the genre. Two-step and rolling are drum and bass patterns.");
    toggle(ui, cx.setter, &cx.p.spread, Rect::from_min_size(pos2(l, t + 46.0), vec2(96.0, 20.0)), "OPEN VOICING", AQUA,
        "Open (drop-2) voicings: the second-highest note drops an octave for a wider, more cinematic sound.");
    let octs = ["Chords −2 oct", "Chords −1 oct", "Chords: middle", "Chords +1 oct", "Chords +2 oct"];
    let cur = octs[(cx.p.octave.value() + 2).clamp(0, 4) as usize];
    if let Some(i) = dd(ui, Rect::from_min_size(pos2(l + 104.0, t + 45.0), vec2(120.0, 22.0)), "octave", cur, &octs, DIM, "Register of the chords (and the MIDI export).") {
        set(cx.setter, &cx.p.octave, i as i32 - 2);
    }
    caption(ui, pos2(l + 2.0, t + 78.0), "KEYBOARD");
    param_dd(ui, cx, Rect::from_min_size(pos2(l + 66.0, t + 72.0), vec2(334.0, 22.0)), "chordin", &cx.p.chord_input, crate::params::input_names(),
        "Play a single note on your keyboard and get the diatonic chord built on it: a fast way to jam ideas in key. Notes outside the scale are ignored.");
}

fn playback_panel(ui: &mut Ui, cx: &Cx, rect: Rect) {
    let inner = panel(ui, rect, "PLAYBACK & ROUTING", AQUA);
    let (l, t) = (inner.left(), inner.top());
    toggle(ui, cx.setter, &cx.p.internal, Rect::from_min_size(pos2(l, t + 2.0), vec2(104.0, 20.0)), "INTERNAL SOUND", AQUA,
        "Play the sketch with the built-in synth. Turn it off when you route the MIDI to your own instruments.");
    toggle(ui, cx.setter, &cx.p.midi_out, Rect::from_min_size(pos2(l + 110.0, t + 2.0), vec2(80.0, 20.0)), "MIDI OUT", ACCENT,
        "Send the notes to the host as MIDI (chords = channel 1, bass = channel 2, melody = channel 3) so you can route them to any instrument. Hosts differ in how they expose plugin MIDI output: if yours cannot, use the MIDI export.");
    let sound_names: Vec<&str> = crate::synth::SOUND_NAMES.to_vec();
    param_dd(ui, cx, Rect::from_min_size(pos2(l + 196.0, t + 1.0), vec2(112.0, 22.0)), "sound", &cx.p.sound, &sound_names, "Sound of the built-in chords.");
    let cols = [("CHORDS", &cx.p.chords_on, &cx.p.chords_vol, MAJOR_C), ("BASS", &cx.p.bass_on, &cx.p.bass_vol, MINOR_C), ("MELODY", &cx.p.melody_on, &cx.p.melody_vol, AQUA)];
    for (i, (label, on, vol, color)) in cols.iter().enumerate() {
        let x = l + i as f32 * 104.0;
        toggle(ui, cx.setter, on, Rect::from_min_size(pos2(x + 4.0, t + 34.0), vec2(90.0, 18.0)), label, *color, "Mute or unmute this part (for the built-in sound, MIDI out and playback).");
        knob(ui, cx.setter, *vol, pos2(x + 49.0, t + 68.0), Kn::S, "LEVEL", *color, "Level of this part in the built-in sound.");
    }
}

// ---------------------------------------------------------------------------------------------
// Export / import
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Which {
    All,
    Chords,
    Bass,
    Melody,
}

impl Which {
    fn suffix(self) -> &'static str {
        match self {
            Which::All => "all",
            Which::Chords => "chords",
            Which::Bass => "bass",
            Which::Melody => "melody",
        }
    }
}

fn midi_bytes(cx: &Cx, which: Which) -> Vec<u8> {
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
    let mut markers = Vec::new();
    let mut t = 0.0;
    for s in &cx.project.sections {
        if !s.items.is_empty() {
            markers.push((t, s.name.clone()));
        }
        t += s.beats();
    }
    write_smf(&SongInfo { bpm: cx.p.bpm.value(), key: cx.key, scale: cx.scale, markers }, &tracks)
}

fn file_stem(cx: &Cx, which: Which) -> String {
    let key = pc_name(cx.key, cx.flats).replace('#', "sharp");
    format!("Chordwave {key} {} {}bpm ({})", cx.scale.name().replace(' ', ""), cx.p.bpm.value().round(), which.suffix())
}

fn save_dialog(cx: &Cx, which: Which) {
    let bytes = midi_bytes(cx, which);
    if which != Which::All && render(&cx.project, &cx.cfg).all().next().is_none() {
        return;
    }
    let name = format!("{}.mid", file_stem(cx, which));
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

/// Write the all-in-one file and one file per part into a folder, then show it.
fn quick_export(cx: &Cx, st: &mut State) {
    let dir = export_folder();
    if let Err(e) = std::fs::create_dir_all(&dir) {
        cx.flash(st, format!("Could not create {}: {e}", dir.display()));
        return;
    }
    let mut written = 0;
    for which in [Which::All, Which::Chords, Which::Bass, Which::Melody] {
        if which != Which::All && !render(&cx.project, &cx.cfg).all().any(|_| true) {
            continue;
        }
        let bytes = midi_bytes(cx, which);
        if std::fs::write(dir.join(format!("{}.mid", file_stem(cx, which))), bytes).is_ok() {
            written += 1;
        }
    }
    open_folder(&dir);
    cx.flash(st, format!("Wrote {written} files to {}", dir.display()));
}

fn import_dialog(cx: &Cx) {
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

fn export_panel(ui: &mut Ui, cx: &Cx, st: &mut State, rect: Rect) {
    let inner = panel(ui, rect, "TO YOUR DAW", ACCENT);
    let (l, t) = (inner.left(), inner.top());
    if let Some(midi) = st.import.clone() {
        import_view(ui, cx, st, &midi, inner);
        return;
    }
    if button(ui, Rect::from_min_size(pos2(l, t + 2.0), vec2(120.0, 24.0)), "save-all", "SAVE MIDI…", ACCENT,
        "Save one MIDI file with the tempo, the key, a marker per section and a separate track for the chords, the bass and the melody. Drag it onto your DAW's timeline.") {
        save_dialog(cx, Which::All);
    }
    for (i, (label, which, tip)) in [("CHORDS", Which::Chords, "Only the chord part"), ("BASS", Which::Bass, "Only the bass part"), ("MELODY", Which::Melody, "Only the melody")].iter().enumerate() {
        if button(ui, Rect::from_min_size(pos2(l + 126.0 + i as f32 * 86.0, t + 2.0), vec2(80.0, 24.0)), &format!("save-{i}"), label, DIM, tip) {
            save_dialog(cx, *which);
        }
    }
    if button(ui, Rect::from_min_size(pos2(l, t + 32.0), vec2(190.0, 24.0)), "quick", "QUICK EXPORT TO FOLDER", AQUA,
        "Write the full file and one file per part into Documents/Chordwave and open the folder: drag them straight into your DAW from there.") {
        quick_export(cx, st);
    }
    if button(ui, Rect::from_min_size(pos2(l + 196.0, t + 32.0), vec2(120.0, 24.0)), "import", "IMPORT MIDI…", DIM,
        "Load a MIDI file (a melody, a bass line or chords) to harmonise it: set its notes as the melody, or let Chordwave detect the chords.") {
        import_dialog(cx);
    }
    let msg = st.message.clone().filter(|(_, at)| cx.time - at < 8.0);
    match msg {
        Some((text, _)) => wrapped(ui, Rect::from_min_size(pos2(l + 2.0, t + 62.0), vec2(inner.width() - 4.0, 34.0)), &text, 10.0, AQUA),
        None => wrapped(ui, Rect::from_min_size(pos2(l + 2.0, t + 62.0), vec2(inner.width() - 4.0, 34.0)),
            "MIDI files carry the exact notes, tempo and key. Drag them from your folder onto a MIDI or instrument track. Or enable MIDI OUT and route the plugin to any synth.", 9.5, DIM),
    }
}

fn import_view(ui: &mut Ui, cx: &Cx, st: &mut State, midi: &MidiIn, inner: Rect) {
    let (l, t) = (inner.left(), inner.top());
    let all: Vec<Note> = midi.tracks.iter().filter(|tr| !tr.drums).flat_map(|tr| tr.notes.clone()).collect();
    let guess = detect_key(&pc_weights(&all)).first().copied();
    let mut head = String::from("Imported MIDI");
    if let Some(b) = midi.bpm {
        head.push_str(&format!("  ·  {:.0} BPM", b));
    }
    ui.painter().text(pos2(l + 2.0, t + 8.0), Align2::LEFT_CENTER, head, FontId::proportional(10.5), TEXT);
    if let Some(g) = guess {
        let label = format!("Key looks like {} {}", pc_name(g.key, use_flats(g.key, g.scale)), g.scale.name());
        ui.painter().text(pos2(l + 150.0, t + 8.0), Align2::LEFT_CENTER, label, FontId::proportional(10.0), AMBER);
        if button(ui, Rect::from_min_size(pos2(l + 300.0, t), vec2(56.0, 17.0)), "imp-key", "USE KEY", AMBER, "Set the key and scale to the detected one.") {
            set(cx.setter, &cx.p.key, g.key as i32);
            set(cx.setter, &cx.p.scale, g.scale.index() as i32);
        }
    }
    if button(ui, Rect::from_min_size(pos2(l + inner.width() - 40.0, t), vec2(40.0, 17.0)), "imp-close", "CLOSE", DIM, "") {
        st.import = None;
        return;
    }
    let tracks: Vec<&TrackIn> = midi.tracks.iter().filter(|tr| !tr.drums).take(4).collect();
    for (i, track) in tracks.iter().enumerate() {
        let y = t + 24.0 + i as f32 * 21.0;
        let label = format!("{}  ({} notes)", ellipsize(&track.name, 26), track.notes.len());
        ui.painter().text(pos2(l + 4.0, y + 10.0), Align2::LEFT_CENTER, label, FontId::proportional(10.0), DIM);
        if button(ui, Rect::from_min_size(pos2(l + 232.0, y), vec2(74.0, 18.0)), &format!("imp-mel{i}"), "AS MELODY", AQUA,
            "Use this track as the melody (the chords are detected for it if the sketch is shorter).") {
            import_as_melody(cx, st, track, midi.bpm);
            st.import = None;
            return;
        }
        if button(ui, Rect::from_min_size(pos2(l + 312.0, y), vec2(78.0, 18.0)), &format!("imp-chd{i}"), "FIND CHORDS", ACCENT,
            "Detect a chord for every bar of this track (melody, bass or chords) and replace the sketch's chords.") {
            find_chords(cx, st, track);
            st.import = None;
            return;
        }
    }
    if midi.tracks.iter().all(|t| t.drums) {
        ui.painter().text(pos2(l + 4.0, t + 40.0), Align2::LEFT_CENTER, "Only drum tracks found.", FontId::proportional(10.0), FAINT);
    }
}

fn import_as_melody(cx: &Cx, st: &mut State, track: &TrackIn, bpm: Option<f32>) {
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

fn find_chords(cx: &Cx, st: &mut State, track: &TrackIn) {
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
