//! Every automatable parameter of Chordwave. The sketch itself (sections, chords, melody) is
//! stored separately by the host as project state, see `project::ProjectCell`.

use crate::project::ProjectCell;
use crate::style::*;
use crate::drums::PATTERNS;
use crate::synth::{BASS_SOUND_NAMES, MELODY_SOUND_NAMES, SOUND_NAMES};
use crate::theory::*;
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

pub const EDITOR_W: u32 = 1120;
pub const EDITOR_H: u32 = 800;

fn names_param(name: &str, default: i32, names: &'static [&'static str]) -> IntParam {
    IntParam::new(name, default, IntRange::Linear { min: 0, max: names.len() as i32 - 1 })
        .with_value_to_string(Arc::new(move |v| names[(v as usize).min(names.len() - 1)].to_string()))
        .with_string_to_value(Arc::new(move |s| {
            let s = s.trim();
            names.iter().position(|n| n.eq_ignore_ascii_case(s)).map(|i| i as i32).or_else(|| s.parse().ok())
        }))
}

const KEY_NAMES: [&str; 12] = SHARP;
const SCALE_NAMES: [&str; 10] = ["Major", "Minor", "Harmonic Minor", "Melodic Minor", "Dorian", "Phrygian", "Lydian", "Mixolydian", "Locrian", "Phrygian Dominant"];
const GENRE_NAMES: [&str; 8] = ["Any", "Synthwave", "Drum & Bass", "Liquid DnB", "Pop", "Cinematic", "Lo-fi / Jazz", "Trance / House"];
const CHORD_STYLE_NAMES: [&str; 7] = ["Sustained pads", "8th pulse", "Offbeat stabs", "Gated 16ths", "Syncopated stabs", "Arp up (16th)", "Arp up / down"];
const BASS_STYLE_NAMES: [&str; 7] = ["Off", "Sustained root", "Driving 8ths", "Octave bounce", "Offbeat root", "DnB two-step", "DnB rolling"];
const INPUT_NAMES: [&str; 4] = ["Off (play notes)", "Chords: triads", "Chords: 7ths", "Chords: 9ths"];

#[derive(Params)]
pub struct ChordwaveParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    /// The chord sections and the melody.
    #[persist = "project"]
    pub project: ProjectCell,

    #[id = "gain"]
    pub gain: FloatParam,

    // Harmony
    #[id = "key"]
    pub key: IntParam,
    #[id = "scale"]
    pub scale: IntParam,
    #[id = "genre"]
    pub genre: IntParam,
    /// Tempo of the standalone app and of the built-in clock when the host does not provide one.
    #[id = "bpm"]
    pub bpm: FloatParam,

    // Parts
    #[id = "cstyle"]
    pub chord_style: IntParam,
    #[id = "bstyle"]
    pub bass_style: IntParam,
    #[id = "spread"]
    pub spread: BoolParam,
    /// Shifts the chords (and their MIDI export) up or down by octaves.
    #[id = "octave"]
    pub octave: IntParam,
    #[id = "mdens"]
    pub mel_density: FloatParam,
    /// Shifts the range of generated melodies by octaves.
    #[id = "moct"]
    pub mel_octave: IntParam,

    // Playback
    #[id = "repeat"]
    pub repeat: BoolParam,
    #[id = "internal"]
    pub internal: BoolParam,
    #[id = "midiout"]
    pub midi_out: BoolParam,
    #[id = "sound"]
    pub sound: IntParam,
    #[id = "bsound"]
    pub bass_sound: IntParam,
    #[id = "msound"]
    pub melody_sound: IntParam,
    /// 0 = the genre's own patterns, 1.. = one of Neon Break's patterns.
    #[id = "dpat"]
    pub drum_pattern: IntParam,
    #[id = "dfills"]
    pub drum_fills: BoolParam,
    #[id = "drums_on"]
    pub drums_on: BoolParam,
    #[id = "drums_vol"]
    pub drums_vol: FloatParam,
    #[id = "chords_on"]
    pub chords_on: BoolParam,
    #[id = "bass_on"]
    pub bass_on: BoolParam,
    #[id = "mel_on"]
    pub melody_on: BoolParam,
    #[id = "chords_vol"]
    pub chords_vol: FloatParam,
    #[id = "bass_vol"]
    pub bass_vol: FloatParam,
    #[id = "mel_vol"]
    pub melody_vol: FloatParam,
    /// Turns single notes played on a keyboard into diatonic chords.
    #[id = "chord_in"]
    pub chord_input: IntParam,
}

impl Default for ChordwaveParams {
    fn default() -> Self {
        let vol = |name: &str| {
            FloatParam::new(name, 1.0, FloatRange::Linear { min: 0.0, max: 1.5 })
                .with_unit("")
                .with_value_to_string(formatters::v2s_f32_rounded(2))
        };
        Self {
            editor_state: EguiState::from_size(EDITOR_W, EDITOR_H),
            project: ProjectCell::default(),
            gain: FloatParam::new("Volume", util::db_to_gain(-3.0), FloatRange::Skewed {
                min: util::db_to_gain(-36.0),
                max: util::db_to_gain(6.0),
                factor: FloatRange::gain_skew_factor(-36.0, 6.0),
            })
            .with_smoother(SmoothingStyle::Logarithmic(30.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            key: names_param("Key", 9, &KEY_NAMES),
            scale: names_param("Scale", Scale::Minor.index() as i32, &SCALE_NAMES),
            genre: names_param("Genre", Genre::Synthwave.index() as i32, &GENRE_NAMES),
            bpm: FloatParam::new("Tempo", 110.0, FloatRange::Linear { min: 50.0, max: 220.0 })
                .with_unit(" BPM")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),

            chord_style: names_param("Chord style", ChordStyle::Gated.index() as i32, &CHORD_STYLE_NAMES),
            bass_style: names_param("Bass style", BassStyle::Eighths.index() as i32, &BASS_STYLE_NAMES),
            spread: BoolParam::new("Open voicing", false),
            octave: IntParam::new("Chord octave", 0, IntRange::Linear { min: -2, max: 2 }),
            mel_density: FloatParam::new("Melody density", 0.55, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            mel_octave: IntParam::new("Melody octave", 0, IntRange::Linear { min: -1, max: 2 }),

            repeat: BoolParam::new("Loop", true),
            internal: BoolParam::new("Internal sound", true),
            midi_out: BoolParam::new("MIDI out", false),
            sound: names_param("Chord sound", 0, &SOUND_NAMES),
            bass_sound: names_param("Bass sound", 0, &BASS_SOUND_NAMES),
            melody_sound: names_param("Melody sound", 0, &MELODY_SOUND_NAMES),
            drum_pattern: IntParam::new("Drum pattern", 0, IntRange::Linear { min: 0, max: PATTERNS.len() as i32 })
                .with_value_to_string(Arc::new(|v| if v <= 0 { "Auto".to_string() } else { PATTERNS[(v as usize - 1).min(PATTERNS.len() - 1)].name.to_string() })),
            drum_fills: BoolParam::new("Drum fills", true),
            drums_on: BoolParam::new("Drums on", true),
            drums_vol: vol("Drums level"),
            chords_on: BoolParam::new("Chords on", true),
            bass_on: BoolParam::new("Bass on", true),
            melody_on: BoolParam::new("Melody on", true),
            chords_vol: vol("Chords level"),
            bass_vol: vol("Bass level"),
            melody_vol: vol("Melody level"),
            chord_input: names_param("Chord input", 0, &INPUT_NAMES),
        }
    }
}

impl ChordwaveParams {
    pub fn key_pc(&self) -> u8 {
        self.key.value().clamp(0, 11) as u8
    }
    pub fn scale_v(&self) -> Scale {
        Scale::from_index(self.scale.value().max(0) as usize)
    }
    pub fn genre_v(&self) -> Genre {
        Genre::from_index(self.genre.value().max(0) as usize)
    }
    /// How the chords and the bass are turned into notes right now.
    pub fn render_cfg(&self) -> crate::render::RenderCfg {
        let genre = self.genre_v();
        let mut cfg = crate::render::RenderCfg::new(self.key_pc(), self.scale_v(), genre);
        cfg.chord_style = ChordStyle::from_index(self.chord_style.value().max(0) as usize);
        cfg.bass_style = BassStyle::from_index(self.bass_style.value().max(0) as usize);
        cfg.spread = self.spread.value();
        cfg.center = 60 + 12 * self.octave.value();
        cfg.drum_pattern = (self.drum_pattern.value() > 0).then(|| self.drum_pattern.value() as usize - 1);
        cfg.drum_fills = self.drum_fills.value();
        cfg
    }
    /// Range (low, high) of generated melodies.
    pub fn melody_range(&self) -> (i32, i32) {
        let o = 12 * self.mel_octave.value();
        (60 + o, 84 + o)
    }
}

pub fn key_names() -> &'static [&'static str] {
    &KEY_NAMES
}
pub fn scale_names() -> &'static [&'static str] {
    &SCALE_NAMES
}
pub fn input_names() -> &'static [&'static str] {
    &INPUT_NAMES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_tables_match_the_theory_definitions() {
        for (i, d) in SCALES.iter().enumerate() {
            assert_eq!(SCALE_NAMES[i], d.name);
        }
        for (i, g) in GENRES.iter().enumerate() {
            assert_eq!(GENRE_NAMES[i], g.name());
        }
        for (i, s) in CHORD_STYLES.iter().enumerate() {
            assert_eq!(CHORD_STYLE_NAMES[i], s.name());
        }
        for (i, s) in BASS_STYLES.iter().enumerate() {
            assert_eq!(BASS_STYLE_NAMES[i], s.name());
        }
    }

    /// What a DAW does when it saves and reloads a project: serialise the persistent fields, restore them elsewhere.
    #[test]
    fn the_sketch_survives_a_host_save_and_restore() {
        use crate::project::{Note, Sel};
        let original = ChordwaveParams::default();
        original.project.edit(|p| {
            p.insert_item(0, 1, crate::project::ChordItem::new(Chord::new(2, Quality::Min7), 2.0));
            p.melody = vec![Note::new(0.0, 1.5, 72, 99), Note::new(6.0, 0.5, 64, 80)];
            p.sections.push(crate::project::Section { name: "Chorus".into(), items: vec![] });
            p.item_mut(Sel { section: 0, item: 0 }).unwrap().inversion = Some(1);
        });
        let saved = original.serialize_fields();
        assert!(saved.contains_key("project"), "{:?}", saved.keys().collect::<Vec<_>>());

        let restored = ChordwaveParams::default();
        assert_ne!(restored.project.get(), original.project.get());
        restored.deserialize_fields(&saved);
        assert_eq!(restored.project.get(), original.project.get());
        assert_eq!(restored.project.get().melody.len(), 2);
        assert_eq!(restored.project.get().sections[1].name, "Chorus");
    }

    #[test]
    fn defaults_are_a_synthwave_a_minor_sketch() {
        let p = ChordwaveParams::default();
        assert_eq!(p.key.value(), 9);
        assert_eq!(Scale::from_index(p.scale.value() as usize), Scale::Minor);
        assert_eq!(Genre::from_index(p.genre.value() as usize), Genre::Synthwave);
    }
}
