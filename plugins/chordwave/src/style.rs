//! Genres, progression templates and the rhythm patterns used to turn chords into playable parts.
//!
//! Synthwave and drum and bass get the most attention, but nothing is locked: the genre only
//! biases suggestions, templates and generated parts, and "Any" keeps everything neutral.

use crate::project::*;
use crate::theory::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Genre {
    Any,
    Synthwave,
    DrumAndBass,
    Liquid,
    Pop,
    Cinematic,
    LoFi,
    Trance,
}

pub const GENRES: [Genre; 8] = [
    Genre::Any,
    Genre::Synthwave,
    Genre::DrumAndBass,
    Genre::Liquid,
    Genre::Pop,
    Genre::Cinematic,
    Genre::LoFi,
    Genre::Trance,
];

impl Genre {
    pub fn index(self) -> usize {
        GENRES.iter().position(|g| *g == self).unwrap_or(0)
    }
    pub fn from_index(i: usize) -> Genre {
        GENRES[i.min(GENRES.len() - 1)]
    }
    pub fn name(self) -> &'static str {
        match self {
            Genre::Any => "Any",
            Genre::Synthwave => "Synthwave",
            Genre::DrumAndBass => "Drum & Bass",
            Genre::Liquid => "Liquid DnB",
            Genre::Pop => "Pop",
            Genre::Cinematic => "Cinematic",
            Genre::LoFi => "Lo-fi / Jazz",
            Genre::Trance => "Trance / House",
        }
    }
    /// Typical tempo, used when a genre is picked in the standalone app.
    pub fn tempo(self) -> f32 {
        match self {
            Genre::Any => 120.0,
            Genre::Synthwave => 100.0,
            Genre::DrumAndBass | Genre::Liquid => 174.0,
            Genre::Pop => 110.0,
            Genre::Cinematic => 80.0,
            Genre::LoFi => 82.0,
            Genre::Trance => 132.0,
        }
    }
    /// The chord and bass styles that suit the genre, as a starting point.
    pub fn default_styles(self) -> (ChordStyle, BassStyle) {
        match self {
            Genre::Any => (ChordStyle::Sustain, BassStyle::Sustain),
            Genre::Synthwave => (ChordStyle::Gated, BassStyle::Eighths),
            Genre::DrumAndBass => (ChordStyle::Sustain, BassStyle::TwoStep),
            Genre::Liquid => (ChordStyle::Stabs, BassStyle::Rolling),
            Genre::Pop => (ChordStyle::Pulse, BassStyle::Sustain),
            Genre::Cinematic => (ChordStyle::Sustain, BassStyle::Sustain),
            Genre::LoFi => (ChordStyle::Stabs, BassStyle::Sustain),
            Genre::Trance => (ChordStyle::Offbeat, BassStyle::Eighths),
        }
    }
    /// MIDI range (low, high) for generated bass lines.
    pub fn bass_range(self) -> (i32, i32) {
        match self {
            Genre::DrumAndBass | Genre::Liquid => (28, 40),
            _ => (33, 45),
        }
    }
    /// How busy a generated melody should be by default (0..1) and how long its notes run.
    pub fn melody_feel(self) -> (f32, f32) {
        match self {
            Genre::Any => (0.5, 1.0),
            Genre::Synthwave => (0.55, 1.0),
            Genre::DrumAndBass => (0.45, 0.5),
            Genre::Liquid => (0.5, 1.0),
            Genre::Pop => (0.5, 1.0),
            Genre::Cinematic => (0.3, 2.0),
            Genre::LoFi => (0.45, 1.0),
            Genre::Trance => (0.6, 0.5),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Rhythm patterns
// ---------------------------------------------------------------------------------------------

/// How the chord track plays each chord.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ChordStyle {
    Sustain,
    Pulse,
    Offbeat,
    Gated,
    Stabs,
    ArpUp,
    ArpUpDown,
}

pub const CHORD_STYLES: [ChordStyle; 7] = [
    ChordStyle::Sustain,
    ChordStyle::Pulse,
    ChordStyle::Offbeat,
    ChordStyle::Gated,
    ChordStyle::Stabs,
    ChordStyle::ArpUp,
    ChordStyle::ArpUpDown,
];

impl ChordStyle {
    pub fn index(self) -> usize {
        CHORD_STYLES.iter().position(|s| *s == self).unwrap_or(0)
    }
    pub fn from_index(i: usize) -> Self {
        CHORD_STYLES[i.min(CHORD_STYLES.len() - 1)]
    }
    pub fn name(self) -> &'static str {
        match self {
            ChordStyle::Sustain => "Sustained pads",
            ChordStyle::Pulse => "8th pulse",
            ChordStyle::Offbeat => "Offbeat stabs",
            ChordStyle::Gated => "Gated 16ths",
            ChordStyle::Stabs => "Syncopated stabs",
            ChordStyle::ArpUp => "Arp up (16th)",
            ChordStyle::ArpUpDown => "Arp up / down",
        }
    }
    /// Block patterns inside one bar: (start, length) in beats. Arps return an empty list.
    pub fn bar_pattern(self) -> &'static [(f32, f32)] {
        match self {
            ChordStyle::Sustain => &[(0.0, 4.0)],
            ChordStyle::Pulse => &[(0.0, 0.5), (0.5, 0.5), (1.0, 0.5), (1.5, 0.5), (2.0, 0.5), (2.5, 0.5), (3.0, 0.5), (3.5, 0.5)],
            ChordStyle::Offbeat => &[(0.5, 0.5), (1.5, 0.5), (2.5, 0.5), (3.5, 0.5)],
            // the classic 3+3+3+3+2+2 gated-pad rhythm of 16th notes
            ChordStyle::Gated => &[(0.0, 0.5), (0.75, 0.5), (1.5, 0.5), (2.25, 0.5), (3.0, 0.5), (3.5, 0.5)],
            ChordStyle::Stabs => &[(0.0, 0.75), (1.5, 0.5), (2.5, 0.5), (3.25, 0.5)],
            ChordStyle::ArpUp | ChordStyle::ArpUpDown => &[],
        }
    }
    pub fn is_arp(self) -> bool {
        matches!(self, ChordStyle::ArpUp | ChordStyle::ArpUpDown)
    }
}

/// How the bass track plays each chord.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BassStyle {
    Off,
    Sustain,
    Eighths,
    Octaves,
    Offbeat,
    TwoStep,
    Rolling,
}

pub const BASS_STYLES: [BassStyle; 7] = [
    BassStyle::Off,
    BassStyle::Sustain,
    BassStyle::Eighths,
    BassStyle::Octaves,
    BassStyle::Offbeat,
    BassStyle::TwoStep,
    BassStyle::Rolling,
];

impl BassStyle {
    pub fn index(self) -> usize {
        BASS_STYLES.iter().position(|s| *s == self).unwrap_or(0)
    }
    pub fn from_index(i: usize) -> Self {
        BASS_STYLES[i.min(BASS_STYLES.len() - 1)]
    }
    pub fn name(self) -> &'static str {
        match self {
            BassStyle::Off => "Off",
            BassStyle::Sustain => "Sustained root",
            BassStyle::Eighths => "Driving 8ths",
            BassStyle::Octaves => "Octave bounce",
            BassStyle::Offbeat => "Offbeat root",
            BassStyle::TwoStep => "DnB two-step",
            BassStyle::Rolling => "DnB rolling",
        }
    }
    /// (start, length, semitones above the root) inside one bar.
    pub fn bar_pattern(self) -> &'static [(f32, f32, i32)] {
        match self {
            BassStyle::Off => &[],
            BassStyle::Sustain => &[(0.0, 4.0, 0)],
            BassStyle::Eighths => &[
                (0.0, 0.5, 0), (0.5, 0.5, 0), (1.0, 0.5, 0), (1.5, 0.5, 0),
                (2.0, 0.5, 0), (2.5, 0.5, 0), (3.0, 0.5, 0), (3.5, 0.5, 0),
            ],
            BassStyle::Octaves => &[
                (0.0, 0.5, 0), (0.5, 0.5, 12), (1.0, 0.5, 0), (1.5, 0.5, 12),
                (2.0, 0.5, 0), (2.5, 0.5, 12), (3.0, 0.5, 0), (3.5, 0.5, 12),
            ],
            BassStyle::Offbeat => &[(0.5, 0.5, 0), (1.5, 0.5, 0), (2.5, 0.5, 0), (3.5, 0.5, 0)],
            // kick on 1 and the "and" of 2, the snare on 2 and 4 stays free for the drums
            BassStyle::TwoStep => &[(0.0, 1.0, 0), (1.5, 0.5, 0), (2.5, 1.0, 0), (3.5, 0.5, 12)],
            BassStyle::Rolling => &[
                (0.0, 0.5, 0), (0.5, 0.25, 0), (1.0, 0.5, 0), (1.75, 0.25, 12),
                (2.0, 0.5, 0), (2.5, 0.5, 0), (3.0, 0.5, 0), (3.5, 0.25, 12),
            ],
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Progression templates
// ---------------------------------------------------------------------------------------------

/// One chord of a template: scale degree (0-based), how it is stacked, and an optional quality
/// override for borrowed chords (for example a major V in natural minor).
pub type Step = (u8, Ext, Option<Quality>);

pub struct Template {
    pub name: &'static str,
    pub genre: Genre,
    pub scale: Scale,
    pub steps: &'static [Step],
    /// Beats each chord lasts.
    pub beats: f32,
    pub why: &'static str,
}

use Ext::*;

pub const TEMPLATES: &[Template] = &[
    // --- Synthwave --------------------------------------------------------------------------
    Template { name: "Outrun", genre: Genre::Synthwave, scale: Scale::Minor, beats: 4.0,
        steps: &[(0, Triad, None), (5, Triad, None), (2, Triad, None), (6, Triad, None)],
        why: "i-VI-III-VII (Am-F-C-G): the synthwave staple. Every chord shares a note with the next, and it ends on VII instead of resolving, so the loop keeps driving forward." },
    Template { name: "Neon Nights", genre: Genre::Synthwave, scale: Scale::Minor, beats: 4.0,
        steps: &[(0, Triad, None), (6, Triad, None), (5, Triad, None), (6, Triad, None)],
        why: "i-VII-VI-VII (Am-G-F-G): the bass walks down and back up one step at a time. Moody and cyclic, ideal under a lead." },
    Template { name: "Night Drive", genre: Genre::Synthwave, scale: Scale::HarmonicMinor, beats: 4.0,
        steps: &[(0, Triad, None), (3, Triad, None), (5, Triad, None), (4, Triad, None)],
        why: "i-iv-VI-V (Am-Dm-F-E): harmonic minor gives a major V chord whose G# leads back to A, so the loop pulls to the tonic with real tension." },
    Template { name: "Retrowave Lift", genre: Genre::Synthwave, scale: Scale::Minor, beats: 4.0,
        steps: &[(5, Seventh, None), (6, Triad, None), (0, Seventh, None), (3, Seventh, None)],
        why: "VImaj7-VII-im7-ivm7: rising VI-VII-i steps lift the energy towards the tonic. The 7ths add the 80s polysynth glow." },
    Template { name: "Stranger Pulse", genre: Genre::Synthwave, scale: Scale::Minor, beats: 4.0,
        steps: &[(0, Triad, None), (2, Triad, None), (6, Triad, None), (5, Triad, None)],
        why: "i-III-VII-VI (Am-C-G-F): all major chords except the tonic, so it feels heroic and a little ominous at once." },
    Template { name: "Miami Sunset", genre: Genre::Synthwave, scale: Scale::Major, beats: 4.0,
        steps: &[(3, Seventh, None), (4, Seventh, None), (2, Seventh, None), (5, Seventh, None)],
        why: "IV-V-iii-vi (Fmaj7-G7-Em7-Am7): the 'Royal Road' of city pop. V should resolve to I but goes to iii instead, a sweet bittersweet deception." },
    Template { name: "Sunset Boulevard", genre: Genre::Synthwave, scale: Scale::Major, beats: 4.0,
        steps: &[(5, Triad, None), (3, Triad, None), (0, Triad, None), (4, Triad, None)],
        why: "vi-IV-I-V: starts on the relative minor so it feels wistful, then lifts into the major chords." },
    Template { name: "Dreamdrive", genre: Genre::Synthwave, scale: Scale::Lydian, beats: 4.0,
        steps: &[(0, Seventh, None), (1, Triad, None), (0, Seventh, None), (6, Seventh, None)],
        why: "Imaj7-II-Imaj7-vii in Lydian: the major II chord contains the raised 4th that gives Lydian its floating, dreamy sound." },
    Template { name: "Phrygian Chase", genre: Genre::Synthwave, scale: Scale::Phrygian, beats: 4.0,
        steps: &[(0, Triad, None), (1, Triad, None), (0, Triad, None), (6, Triad, None)],
        why: "i-bII-i-bVII: the flat-2 chord a half step above the tonic is dark and urgent: action-movie and retro-game music." },
    // --- Drum and bass ---------------------------------------------------------------------
    Template { name: "Roller", genre: Genre::DrumAndBass, scale: Scale::Minor, beats: 8.0,
        steps: &[(0, Triad, None), (6, Triad, None), (5, Triad, None), (6, Triad, None)],
        why: "i-VII-VI-VII with two bars per chord: at 174 BPM a slower harmonic rhythm leaves room for the drums and bass to carry the energy." },
    Template { name: "Minimal Two-Chord", genre: Genre::DrumAndBass, scale: Scale::Minor, beats: 8.0,
        steps: &[(0, Seventh, None), (5, Seventh, None), (0, Seventh, None), (5, Seventh, None)],
        why: "im7-VImaj7: just two chords. Less harmony means more focus on the bassline and the groove, the core of the minimal / techstep approach." },
    Template { name: "Neurofunk Menace", genre: Genre::DrumAndBass, scale: Scale::Phrygian, beats: 8.0,
        steps: &[(0, Triad, None), (1, Triad, None), (0, Triad, None), (1, Triad, None)],
        why: "i-bII-i-bII: the half-step shift from tonic to bII is the signature neurofunk tension." },
    Template { name: "Dark Jungle", genre: Genre::DrumAndBass, scale: Scale::HarmonicMinor, beats: 8.0,
        steps: &[(0, Triad, None), (3, Triad, None), (0, Triad, None), (4, Seventh, None)],
        why: "i-iv-i-V7: the dominant seventh (E7 in A minor) from harmonic minor makes a dark, dramatic turnaround before the loop restarts." },
    Template { name: "Dorian Vamp", genre: Genre::DrumAndBass, scale: Scale::Dorian, beats: 8.0,
        steps: &[(0, Seventh, None), (3, Seventh, None), (0, Seventh, None), (6, Seventh, None)],
        why: "im7-IV7: Dorian's major IV chord is the 'soulful' colour and works on a half-time or rolling groove." },
    Template { name: "Liquid Soul", genre: Genre::Liquid, scale: Scale::Minor, beats: 8.0,
        steps: &[(0, Ninth, None), (3, Ninth, None), (5, Ninth, None), (6, Ninth, None)],
        why: "im9-ivm9-VImaj9-VII9: the added 9ths smooth every chord, the sound of liquid funk. Roots move by fourths and steps, so it flows." },
    Template { name: "Liquid Float", genre: Genre::Liquid, scale: Scale::Major, beats: 8.0,
        steps: &[(0, Seventh, None), (2, Seventh, None), (1, Seventh, None), (4, Seventh, None)],
        why: "Imaj7-iiim7-iim7-V7: all chords in 7th form, descending and gently suspended before the dominant." },
    // --- Pop / general ---------------------------------------------------------------------
    Template { name: "Four Chords", genre: Genre::Pop, scale: Scale::Major, beats: 4.0,
        steps: &[(0, Triad, None), (4, Triad, None), (5, Triad, None), (3, Triad, None)],
        why: "I-V-vi-IV: the most used progression in pop. It alternates stability and lift and loops without ever feeling finished." },
    Template { name: "50s Turnaround", genre: Genre::Pop, scale: Scale::Major, beats: 4.0,
        steps: &[(0, Triad, None), (5, Triad, None), (3, Triad, None), (4, Triad, None)],
        why: "I-vi-IV-V: the doo-wop loop, bass descending by thirds and ending on a dominant that wants to go home." },
    Template { name: "Canon", genre: Genre::Pop, scale: Scale::Major, beats: 4.0,
        steps: &[(0, Triad, None), (4, Triad, None), (5, Triad, None), (2, Triad, None), (3, Triad, None), (0, Triad, None), (3, Triad, None), (4, Triad, None)],
        why: "I-V-vi-iii-IV-I-IV-V: Pachelbel's canon, a bass line that falls step by step through the key." },
    Template { name: "Andalusian", genre: Genre::Pop, scale: Scale::Minor, beats: 4.0,
        steps: &[(0, Triad, None), (6, Triad, None), (5, Triad, None), (4, Triad, Some(Quality::Maj))],
        why: "i-VII-VI-V (Am-G-F-E): a stepwise descent ending on a major V borrowed from harmonic minor: flamenco drama." },
    // --- Cinematic -------------------------------------------------------------------------
    Template { name: "Epic Rise", genre: Genre::Cinematic, scale: Scale::Minor, beats: 8.0,
        steps: &[(5, Triad, None), (6, Triad, None), (0, Triad, None), (0, Sus2, None)],
        why: "VI-VII-i-isus2: a rising line to the tonic, then a suspended chord that stays open for the next section." },
    Template { name: "Wonder", genre: Genre::Cinematic, scale: Scale::Lydian, beats: 8.0,
        steps: &[(0, Triad, None), (1, Triad, None), (0, Triad, None), (4, Triad, None)],
        why: "I-II-I-V in Lydian: the major II chord is the 'sense of wonder' sound used in film scores." },
    // --- Lo-fi / Jazz ----------------------------------------------------------------------
    Template { name: "Turnaround", genre: Genre::LoFi, scale: Scale::Major, beats: 4.0,
        steps: &[(1, Seventh, None), (4, Seventh, None), (0, Seventh, None), (5, Seventh, None)],
        why: "iim7-V7-Imaj7-vim7: the jazz ii-V-I followed by vi to loop back to ii. Roots fall by fifths, the strongest motion in tonal music." },
    Template { name: "Lo-fi Rain", genre: Genre::LoFi, scale: Scale::Major, beats: 4.0,
        steps: &[(0, Seventh, None), (5, Seventh, None), (1, Seventh, None), (4, Seventh, None)],
        why: "Imaj7-vim7-iim7-V7: the 'I-vi-ii-V' circle, all in 7ths for the warm, hazy lo-fi hip-hop sound." },
    Template { name: "Minor ii-V-i", genre: Genre::LoFi, scale: Scale::HarmonicMinor, beats: 4.0,
        steps: &[(1, Seventh, None), (4, Seventh, None), (0, Seventh, None), (5, Seventh, None)],
        why: "iiø7-V7-im(maj7)-VImaj7: the minor ii-V-i with harmonic minor's dominant V7 resolving to a mysterious minor-major tonic." },
    // --- Trance / House --------------------------------------------------------------------
    Template { name: "Trance Anthem", genre: Genre::Trance, scale: Scale::Minor, beats: 4.0,
        steps: &[(5, Triad, None), (2, Triad, None), (6, Triad, None), (0, Triad, None)],
        why: "VI-III-VII-i: major chords lifting to the minor tonic: the euphoric breakdown loop." },
    Template { name: "Piano House", genre: Genre::Trance, scale: Scale::Mixolydian, beats: 4.0,
        steps: &[(0, Triad, None), (6, Triad, None), (3, Triad, None), (0, Triad, None)],
        why: "I-bVII-IV-I: Mixolydian's flat-7 chord gives a laid-back, soulful house feel." },
];

/// Templates for a genre; `Any` returns everything.
pub fn templates_for(genre: Genre) -> Vec<&'static Template> {
    TEMPLATES.iter().filter(|t| genre == Genre::Any || t.genre == genre).collect()
}

impl Template {
    pub fn chords(&self, key: u8) -> Vec<Chord> {
        self.steps
            .iter()
            .map(|&(deg, ext, over)| {
                let mut c = diatonic_chord(key, self.scale, deg as usize, ext);
                if let Some(q) = over {
                    c.quality = q;
                }
                c
            })
            .collect()
    }

    pub fn section(&self, key: u8, name: &str) -> Section {
        Section { name: name.to_string(), items: self.chords(key).into_iter().map(|c| ChordItem::new(c, self.beats)).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outrun_in_a_minor_is_am_f_c_g() {
        let t = TEMPLATES.iter().find(|t| t.name == "Outrun").unwrap();
        let names: Vec<_> = t.chords(9).iter().map(|c| c.name(false)).collect();
        assert_eq!(names, ["Am", "F", "C", "G"]);
    }

    #[test]
    fn andalusian_has_a_major_v() {
        let t = TEMPLATES.iter().find(|t| t.name == "Andalusian").unwrap();
        let names: Vec<_> = t.chords(9).iter().map(|c| c.name(false)).collect();
        assert_eq!(names, ["Am", "G", "F", "E"]);
    }

    #[test]
    fn every_genre_has_templates_and_all_chords_exist() {
        for g in GENRES.iter().filter(|g| **g != Genre::Any) {
            assert!(!templates_for(*g).is_empty(), "{g:?} has no templates");
        }
        for t in TEMPLATES {
            assert!(!t.chords(0).is_empty());
        }
    }

    #[test]
    fn style_indices_round_trip() {
        for (i, s) in CHORD_STYLES.iter().enumerate() {
            assert_eq!(s.index(), i);
        }
        for (i, s) in BASS_STYLES.iter().enumerate() {
            assert_eq!(s.index(), i);
        }
        for g in GENRES {
            assert_eq!(Genre::from_index(g.index()), g);
        }
    }
}
