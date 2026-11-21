//! Music theory core: scales, chords, diatonic harmony, roman numerals and voicings.
//!
//! Everything here is plain data and pure functions (no plugin or UI types), so it is easy to test.
//! Pitch classes are `0..12` with 0 = C. MIDI notes are `u8`, 60 = middle C (C4 in most DAWs,
//! C5 in FL Studio).

use serde::{Deserialize, Serialize};

pub const SHARP: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
pub const FLAT: [&str; 12] = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"];

/// Name of a pitch class, spelled with sharps or flats.
pub fn pc_name(pc: u8, flats: bool) -> &'static str {
    if flats { FLAT[(pc % 12) as usize] } else { SHARP[(pc % 12) as usize] }
}

/// MIDI note name in the "C4 = 60" convention, for example `C#3`.
pub fn midi_name(note: u8) -> String {
    format!("{}{}", SHARP[(note % 12) as usize], note as i32 / 12 - 1)
}

// ---------------------------------------------------------------------------------------------
// Scales
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Scale {
    Major,
    Minor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    PhrygianDominant,
}

pub struct ScaleDef {
    pub scale: Scale,
    pub name: &'static str,
    pub intervals: [u8; 7],
    /// Semitones from the tonic of the parent major scale up to this mode's tonic (for spelling).
    pub parent_offset: u8,
    pub blurb: &'static str,
}

pub const SCALES: [ScaleDef; 10] = [
    ScaleDef { scale: Scale::Major, name: "Major", intervals: [0, 2, 4, 5, 7, 9, 11], parent_offset: 0,
        blurb: "Bright and resolved. Classic pop, uplifting trance and sunset synthwave." },
    ScaleDef { scale: Scale::Minor, name: "Minor", intervals: [0, 2, 3, 5, 7, 8, 10], parent_offset: 9,
        blurb: "Natural minor (Aeolian). The home of synthwave and most drum and bass: dark but warm." },
    ScaleDef { scale: Scale::HarmonicMinor, name: "Harmonic Minor", intervals: [0, 2, 3, 5, 7, 8, 11], parent_offset: 9,
        blurb: "Minor with a raised 7th: gives a real V chord (major / dominant) that pulls back to i. Dramatic, neo-classical." },
    ScaleDef { scale: Scale::MelodicMinor, name: "Melodic Minor", intervals: [0, 2, 3, 5, 7, 9, 11], parent_offset: 9,
        blurb: "Minor with raised 6th and 7th. Smooth, jazzy and slightly mysterious (minor-major tonic)." },
    ScaleDef { scale: Scale::Dorian, name: "Dorian", intervals: [0, 2, 3, 5, 7, 9, 10], parent_offset: 2,
        blurb: "Minor with a raised 6th: the bright minor. Funky, soulful, very common in liquid drum and bass." },
    ScaleDef { scale: Scale::Phrygian, name: "Phrygian", intervals: [0, 1, 3, 5, 7, 8, 10], parent_offset: 4,
        blurb: "Minor with a flat 2nd: tense and dark. The bII chord is the classic neurofunk / metal sound." },
    ScaleDef { scale: Scale::Lydian, name: "Lydian", intervals: [0, 2, 4, 6, 7, 9, 11], parent_offset: 5,
        blurb: "Major with a raised 4th: dreamy and floating. Great for cinematic and chill synthwave." },
    ScaleDef { scale: Scale::Mixolydian, name: "Mixolydian", intervals: [0, 2, 4, 5, 7, 9, 10], parent_offset: 7,
        blurb: "Major with a flat 7th: laid-back, bluesy, rock and house." },
    ScaleDef { scale: Scale::Locrian, name: "Locrian", intervals: [0, 1, 3, 5, 6, 8, 10], parent_offset: 11,
        blurb: "Minor with flat 2nd and flat 5th: unstable, rarely used as a home key." },
    ScaleDef { scale: Scale::PhrygianDominant, name: "Phrygian Dominant", intervals: [0, 1, 4, 5, 7, 8, 10], parent_offset: 4,
        blurb: "Fifth mode of harmonic minor: exotic, Spanish / Middle-Eastern flavour with a major tonic chord." },
];

impl Scale {
    pub fn index(self) -> usize {
        SCALES.iter().position(|d| d.scale == self).unwrap_or(0)
    }
    pub fn from_index(i: usize) -> Scale {
        SCALES[i.min(SCALES.len() - 1)].scale
    }
    pub fn def(self) -> &'static ScaleDef {
        &SCALES[self.index()]
    }
    pub fn name(self) -> &'static str {
        self.def().name
    }
    pub fn intervals(self) -> [u8; 7] {
        self.def().intervals
    }
    /// Does the tonic chord of this scale have a minor third?
    pub fn is_minor_like(self) -> bool {
        self.intervals()[2] == 3
    }
}

/// Should this key be spelled with flats (Bb, Eb...) rather than sharps?
pub fn use_flats(key: u8, scale: Scale) -> bool {
    let parent = (key as i32 + 12 - scale.def().parent_offset as i32).rem_euclid(12) as u8;
    matches!(parent, 5 | 10 | 3 | 8 | 1 | 6)
}

/// Semitone position of scale degree `degree` (0-based, may exceed 6 to continue into the next octave).
pub fn degree_semis(scale: Scale, degree: usize) -> i32 {
    scale.intervals()[degree % 7] as i32 + 12 * (degree / 7) as i32
}

pub fn scale_mask(key: u8, scale: Scale) -> [bool; 12] {
    let mut m = [false; 12];
    for i in scale.intervals() {
        m[((key + i) % 12) as usize] = true;
    }
    m
}

pub fn in_scale(key: u8, scale: Scale, pc: u8) -> bool {
    scale_mask(key, scale)[(pc % 12) as usize]
}

/// Move `steps` places along the circle of fifths: C -> G -> D ... (negative: C -> F -> Bb ...).
pub fn fifths_key(key: u8, steps: i32) -> u8 {
    (key as i32 + 7 * steps).rem_euclid(12) as u8
}

// ---------------------------------------------------------------------------------------------
// Chord qualities
// ---------------------------------------------------------------------------------------------

/// Colour class used in the palette: major-ish, minor-ish or everything else.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Major,
    Minor,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Quality {
    Maj,
    Min,
    Dim,
    Aug,
    Sus2,
    Sus4,
    Power,
    Six,
    MinSix,
    Maj7,
    Dom7,
    Min7,
    MinMaj7,
    Dim7,
    HalfDim7,
    AugMaj7,
    Dom7Sus4,
    Add9,
    MinAdd9,
    Maj9,
    Dom9,
    Min9,
    Dom7b9,
    Min11,
    Maj7s11,
}

pub struct QualityDef {
    pub q: Quality,
    pub symbol: &'static str,
    pub intervals: &'static [u8],
    pub kind: Kind,
}

macro_rules! q {
    ($q:ident, $s:expr, [$($i:expr),*], $k:ident) => {
        QualityDef { q: Quality::$q, symbol: $s, intervals: &[$($i),*], kind: Kind::$k }
    };
}

pub const QUALITIES: [QualityDef; 25] = [
    q!(Maj, "", [0, 4, 7], Major),
    q!(Min, "m", [0, 3, 7], Minor),
    q!(Dim, "dim", [0, 3, 6], Other),
    q!(Aug, "aug", [0, 4, 8], Other),
    q!(Sus2, "sus2", [0, 2, 7], Other),
    q!(Sus4, "sus4", [0, 5, 7], Other),
    q!(Power, "5", [0, 7], Other),
    q!(Six, "6", [0, 4, 7, 9], Major),
    q!(MinSix, "m6", [0, 3, 7, 9], Minor),
    q!(Maj7, "maj7", [0, 4, 7, 11], Major),
    q!(Dom7, "7", [0, 4, 7, 10], Major),
    q!(Min7, "m7", [0, 3, 7, 10], Minor),
    q!(MinMaj7, "mMaj7", [0, 3, 7, 11], Minor),
    q!(Dim7, "dim7", [0, 3, 6, 9], Other),
    q!(HalfDim7, "m7b5", [0, 3, 6, 10], Other),
    q!(AugMaj7, "maj7#5", [0, 4, 8, 11], Other),
    q!(Dom7Sus4, "7sus4", [0, 5, 7, 10], Other),
    q!(Add9, "add9", [0, 4, 7, 14], Major),
    q!(MinAdd9, "m(add9)", [0, 3, 7, 14], Minor),
    q!(Maj9, "maj9", [0, 4, 7, 11, 14], Major),
    q!(Dom9, "9", [0, 4, 7, 10, 14], Major),
    q!(Min9, "m9", [0, 3, 7, 10, 14], Minor),
    q!(Dom7b9, "7b9", [0, 4, 7, 10, 13], Major),
    q!(Min11, "m11", [0, 3, 7, 10, 14, 17], Minor),
    q!(Maj7s11, "maj7#11", [0, 4, 7, 11, 14, 18], Major),
];

impl Quality {
    pub fn def(self) -> &'static QualityDef {
        QUALITIES.iter().find(|d| d.q == self).expect("every quality has a definition")
    }
    pub fn symbol(self) -> &'static str {
        self.def().symbol
    }
    pub fn intervals(self) -> &'static [u8] {
        self.def().intervals
    }
    pub fn kind(self) -> Kind {
        self.def().kind
    }
    /// Find the quality with exactly these intervals above the root.
    pub fn identify(intervals: &[u8]) -> Option<Quality> {
        QUALITIES.iter().find(|d| d.intervals == intervals).map(|d| d.q)
    }
}

// ---------------------------------------------------------------------------------------------
// Chords
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Chord {
    pub root: u8,
    pub quality: Quality,
    /// Slash chord: a different note in the bass (pitch class).
    pub bass: Option<u8>,
}

impl Chord {
    pub fn new(root: u8, quality: Quality) -> Self {
        Self { root: root % 12, quality, bass: None }
    }

    /// Chord tones as semitone offsets above the root, ascending (9ths and 11ths above 12).
    pub fn intervals(&self) -> &'static [u8] {
        self.quality.intervals()
    }

    /// Distinct pitch classes of the chord (without the slash bass).
    pub fn pitch_classes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = self.intervals().iter().map(|i| (self.root + i) % 12).collect();
        v.dedup();
        v
    }

    pub fn contains_pc(&self, pc: u8) -> bool {
        self.pitch_classes().contains(&(pc % 12))
    }

    pub fn name(&self, flats: bool) -> String {
        let mut s = format!("{}{}", pc_name(self.root, flats), self.quality.symbol());
        if let Some(b) = self.bass {
            if b != self.root {
                s.push('/');
                s.push_str(pc_name(b, flats));
            }
        }
        s
    }

    pub fn kind(&self) -> Kind {
        self.quality.kind()
    }

    /// Are all the notes of this chord inside the scale? (A hollow circle in the palette otherwise.)
    pub fn in_key(&self, key: u8, scale: Scale) -> bool {
        let mask = scale_mask(key, scale);
        self.pitch_classes().iter().all(|&p| mask[p as usize])
    }

    pub fn with_bass(mut self, bass: Option<u8>) -> Self {
        self.bass = bass.filter(|b| *b != self.root);
        self
    }
}

/// How a diatonic chord is stacked or altered.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Ext {
    Triad,
    Seventh,
    Ninth,
    Eleventh,
    Sus2,
    Sus4,
    Add9,
    Sixth,
    Power,
}

pub const EXTS: [Ext; 9] =
    [Ext::Triad, Ext::Seventh, Ext::Ninth, Ext::Eleventh, Ext::Sus2, Ext::Sus4, Ext::Add9, Ext::Sixth, Ext::Power];

impl Ext {
    pub fn label(self) -> &'static str {
        match self {
            Ext::Triad => "Triad",
            Ext::Seventh => "7th",
            Ext::Ninth => "9th",
            Ext::Eleventh => "11th",
            Ext::Sus2 => "sus2",
            Ext::Sus4 => "sus4",
            Ext::Add9 => "add9",
            Ext::Sixth => "6th",
            Ext::Power => "5th",
        }
    }
}

/// The chord on scale degree `degree` (0..7) of `key scale`, built by stacking scale thirds.
/// Extensions that would produce an unnamed chord fall back to the next simpler one.
pub fn diatonic_chord(key: u8, scale: Scale, degree: usize, ext: Ext) -> Chord {
    let d = degree % 7;
    let root = ((key as i32 + degree_semis(scale, d)).rem_euclid(12)) as u8;
    let stack = |count: usize| -> Vec<u8> {
        (0..count).map(|k| (degree_semis(scale, d + 2 * k) - degree_semis(scale, d)) as u8).collect()
    };
    let stacked = |count: usize| Quality::identify(&stack(count));
    let triad = stacked(3).unwrap_or(Quality::Maj);
    let quality = match ext {
        Ext::Triad => triad,
        Ext::Seventh => stacked(4).unwrap_or(triad),
        Ext::Ninth => stacked(5).or_else(|| stacked(4)).unwrap_or(triad),
        Ext::Eleventh => stacked(6).or_else(|| stacked(5)).or_else(|| stacked(4)).unwrap_or(triad),
        Ext::Sus2 => Quality::Sus2,
        Ext::Sus4 => Quality::Sus4,
        Ext::Power => Quality::Power,
        Ext::Add9 => match triad {
            Quality::Maj => Quality::Add9,
            Quality::Min => Quality::MinAdd9,
            other => other,
        },
        Ext::Sixth => match triad {
            Quality::Maj => Quality::Six,
            Quality::Min => Quality::MinSix,
            other => other,
        },
    };
    Chord::new(root, quality)
}

/// A chord's degree in the scale, if its root belongs to it.
pub fn degree_of(key: u8, scale: Scale, pc: u8) -> Option<usize> {
    let off = ((pc as i32 - key as i32).rem_euclid(12)) as u8;
    scale.intervals().iter().position(|&i| i == off)
}

const ROMAN: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
const CHROMATIC_ROMAN: [&str; 12] = ["I", "bII", "II", "bIII", "III", "IV", "#IV", "V", "bVI", "VI", "bVII", "VII"];

/// Roman numeral analysis: `i`, `bVII`, `V7`, `iiø7` ... Upper case for major-ish chords.
pub fn roman(key: u8, scale: Scale, chord: &Chord) -> String {
    let off = ((chord.root as i32 - key as i32).rem_euclid(12)) as usize;
    let base = match degree_of(key, scale, chord.root) {
        Some(d) => ROMAN[d].to_string(),
        None => CHROMATIC_ROMAN[off].to_string(),
    };
    let lower = matches!(chord.quality.kind(), Kind::Minor) || matches!(chord.quality, Quality::Dim | Quality::Dim7 | Quality::HalfDim7);
    let numeral = if lower {
        // keep a leading accidental, lower-case the numeral
        let (acc, rest) = base.split_at(base.chars().take_while(|c| *c == 'b' || *c == '#').count());
        format!("{acc}{}", rest.to_lowercase())
    } else {
        base
    };
    let suffix = match chord.quality {
        Quality::Maj | Quality::Min => "",
        Quality::Dim => "°",
        Quality::Aug => "+",
        Quality::Sus2 => "sus2",
        Quality::Sus4 => "sus4",
        Quality::Power => "5",
        Quality::Six | Quality::MinSix => "6",
        Quality::Maj7 => "maj7",
        Quality::Dom7 | Quality::Min7 => "7",
        Quality::MinMaj7 => "(maj7)",
        Quality::Dim7 => "°7",
        Quality::HalfDim7 => "ø7",
        Quality::AugMaj7 => "+maj7",
        Quality::Dom7Sus4 => "7sus4",
        Quality::Add9 | Quality::MinAdd9 => "add9",
        Quality::Maj9 => "maj9",
        Quality::Dom9 | Quality::Min9 => "9",
        Quality::Dom7b9 => "7b9",
        Quality::Min11 => "11",
        Quality::Maj7s11 => "maj7#11",
    };
    format!("{numeral}{suffix}")
}

/// Harmonic function: where a chord sits in the tension and release cycle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Function {
    /// Home and rest: the chord feels settled.
    Tonic,
    /// Moving away from home and building towards tension.
    Predominant,
    /// Tension that wants to resolve back to the tonic.
    Dominant,
    /// Not in the scale: a borrowed chord that adds colour.
    Colour,
}

impl Function {
    pub fn label(self) -> &'static str {
        match self {
            Function::Tonic => "Tonic",
            Function::Predominant => "Predominant",
            Function::Dominant => "Dominant",
            Function::Colour => "Colour",
        }
    }
    pub fn explain(self) -> &'static str {
        match self {
            Function::Tonic => "feels like home: stable, a good place to start or end",
            Function::Predominant => "leaves home and builds anticipation: leads to the dominant or back",
            Function::Dominant => "tension that wants to resolve to the tonic",
            Function::Colour => "outside the key: a borrowed chord that adds a new colour",
        }
    }
}

pub fn function_of(key: u8, scale: Scale, chord: &Chord) -> Function {
    if !in_scale(key, scale, chord.root) {
        return Function::Colour;
    }
    let d = degree_of(key, scale, chord.root).unwrap_or(0);
    let minor = scale.is_minor_like();
    match d {
        0 | 2 => Function::Tonic,
        1 | 3 => Function::Predominant,
        4 | 6 => Function::Dominant,
        5 => if minor { Function::Predominant } else { Function::Tonic },
        _ => Function::Tonic,
    }
}

// ---------------------------------------------------------------------------------------------
// Voicing
// ---------------------------------------------------------------------------------------------

/// Notes of a chord as MIDI numbers, ascending.
///
/// `inversion` rotates the lowest notes up an octave, `spread` drops the second-highest note by an
/// octave (an "open" voicing), and the whole chord is placed so it sits around `center`.
pub fn chord_notes(chord: &Chord, inversion: u8, spread: bool, center: i32) -> Vec<u8> {
    let mut tones: Vec<i32> = chord.intervals().iter().map(|&i| chord.root as i32 + i as i32).collect();
    // Fold wide extensions (9th, 11th) down so the close voicing stays compact when inverted.
    let n = tones.len();
    for _ in 0..(inversion as usize % n.max(1)) {
        let low = tones.remove(0);
        tones.push(low + 12);
    }
    tones.sort_unstable();
    if spread && tones.len() >= 3 {
        let i = tones.len() - 2;
        tones[i] -= 12;
        tones.sort_unstable();
    }
    let mean = tones.iter().sum::<i32>() as f32 / tones.len() as f32;
    // Choose the octave shift that puts the mean closest to the centre.
    let shift = ((center as f32 - mean) / 12.0).round() as i32 * 12;
    let mut out: Vec<i32> = tones.iter().map(|t| t + shift).collect();
    if let Some(b) = chord.bass {
        let lowest = out[0];
        let mut note = lowest - 1;
        while (note.rem_euclid(12)) as u8 != b % 12 {
            note -= 1;
        }
        out.insert(0, note);
    }
    out.iter().map(|&n| n.clamp(0, 127) as u8).collect()
}

/// Choose the inversion (and octave) that moves the least from the previous chord: smooth voice leading.
pub fn voice_lead(prev: &[u8], chord: &Chord, spread: bool, center: i32) -> (Vec<u8>, u8) {
    let count = chord.intervals().len().max(1);
    if prev.is_empty() {
        return (chord_notes(chord, 0, spread, center), 0);
    }
    let prev_mean = prev.iter().map(|&n| n as f32).sum::<f32>() / prev.len() as f32;
    let mut best: Option<(f32, Vec<u8>, u8)> = None;
    for inv in 0..count as u8 {
        for octave in [-12i32, 0, 12] {
            let notes = chord_notes(chord, inv, spread, center + octave);
            // Compare each new note with the nearest previous note.
            let cost: f32 = notes
                .iter()
                .map(|&n| prev.iter().map(|&p| (n as i32 - p as i32).abs()).min().unwrap_or(0) as f32)
                .sum::<f32>()
                / notes.len() as f32;
            let mean = notes.iter().map(|&n| n as f32).sum::<f32>() / notes.len() as f32;
            let score = cost + 0.35 * (mean - prev_mean).abs() + 0.05 * (mean - center as f32).abs();
            if best.as_ref().map_or(true, |b| score < b.0) {
                best = Some((score, notes, inv));
            }
        }
    }
    let (_, notes, inv) = best.unwrap();
    (notes, inv)
}

/// Lowest note of the given pitch class inside `lo..=hi` (falls back to the nearest to `lo`).
pub fn pc_in_range(pc: u8, lo: i32, hi: i32) -> i32 {
    for n in lo..=hi {
        if n.rem_euclid(12) as u8 == pc % 12 {
            return n;
        }
    }
    lo + (pc as i32 - lo).rem_euclid(12)
}

/// All scale tones in `lo..=hi`.
pub fn scale_notes(key: u8, scale: Scale, lo: i32, hi: i32) -> Vec<u8> {
    let mask = scale_mask(key, scale);
    (lo..=hi).filter(|n| mask[n.rem_euclid(12) as usize]).map(|n| n as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(key: u8, scale: Scale, ext: Ext) -> Vec<String> {
        let flats = use_flats(key, scale);
        (0..7).map(|d| diatonic_chord(key, scale, d, ext).name(flats)).collect()
    }

    #[test]
    fn c_major_triads_and_sevenths() {
        assert_eq!(names(0, Scale::Major, Ext::Triad), ["C", "Dm", "Em", "F", "G", "Am", "Bdim"]);
        assert_eq!(
            names(0, Scale::Major, Ext::Seventh),
            ["Cmaj7", "Dm7", "Em7", "Fmaj7", "G7", "Am7", "Bm7b5"]
        );
    }

    #[test]
    fn a_minor_chords_match_the_textbook() {
        assert_eq!(names(9, Scale::Minor, Ext::Triad), ["Am", "Bdim", "C", "Dm", "Em", "F", "G"]);
        assert_eq!(
            names(9, Scale::Minor, Ext::Seventh),
            ["Am7", "Bm7b5", "Cmaj7", "Dm7", "Em7", "Fmaj7", "G7"]
        );
    }

    #[test]
    fn harmonic_minor_has_a_dominant_v() {
        let v = diatonic_chord(9, Scale::HarmonicMinor, 4, Ext::Triad);
        assert_eq!(v.name(false), "E");
        assert_eq!(diatonic_chord(9, Scale::HarmonicMinor, 4, Ext::Seventh).name(false), "E7");
        assert_eq!(diatonic_chord(9, Scale::HarmonicMinor, 6, Ext::Seventh).name(false), "G#dim7");
        assert_eq!(diatonic_chord(9, Scale::HarmonicMinor, 2, Ext::Seventh).quality, Quality::AugMaj7);
    }

    #[test]
    fn ninths_and_extensions() {
        assert_eq!(diatonic_chord(0, Scale::Major, 1, Ext::Ninth).name(false), "Dm9");
        assert_eq!(diatonic_chord(0, Scale::Major, 0, Ext::Ninth).name(false), "Cmaj9");
        assert_eq!(diatonic_chord(0, Scale::Major, 4, Ext::Ninth).name(false), "G9");
        assert_eq!(diatonic_chord(0, Scale::Major, 0, Ext::Add9).name(false), "Cadd9");
        assert_eq!(diatonic_chord(0, Scale::Major, 5, Ext::Sixth).name(false), "Am6");
    }

    #[test]
    fn roman_numerals() {
        let c = |d, e| diatonic_chord(9, Scale::Minor, d, e);
        let rn = |ch: &Chord| roman(9, Scale::Minor, ch);
        assert_eq!(rn(&c(0, Ext::Triad)), "i");
        assert_eq!(rn(&c(5, Ext::Triad)), "VI");
        assert_eq!(rn(&c(6, Ext::Seventh)), "VII7");
        assert_eq!(rn(&c(1, Ext::Seventh)), "iiø7");
        // borrowed: a major IV in a minor key is "IV", a bII is "bII"
        assert_eq!(rn(&Chord::new(2, Quality::Maj)), "IV");
        assert_eq!(rn(&Chord::new(10, Quality::Maj)), "bII");
        assert_eq!(roman(0, Scale::Major, &Chord::new(10, Quality::Maj)), "bVII");
    }

    #[test]
    fn functions() {
        let g = Chord::new(7, Quality::Dom7);
        assert_eq!(function_of(0, Scale::Major, &g), Function::Dominant);
        assert_eq!(function_of(0, Scale::Major, &Chord::new(10, Quality::Maj)), Function::Colour);
        assert_eq!(function_of(9, Scale::Minor, &Chord::new(9, Quality::Min)), Function::Tonic);
    }

    #[test]
    fn spelling() {
        assert!(use_flats(5, Scale::Major)); // F major
        assert!(use_flats(2, Scale::Minor)); // D minor (parent F major)
        assert!(!use_flats(9, Scale::Minor)); // A minor
        assert_eq!(Chord::new(10, Quality::Maj).name(true), "Bb");
        assert_eq!(midi_name(60), "C4");
    }

    #[test]
    fn voicing_is_ascending_and_near_center() {
        let c = Chord::new(0, Quality::Maj);
        assert_eq!(chord_notes(&c, 0, false, 60), vec![60, 64, 67]);
        assert_eq!(chord_notes(&c, 1, false, 60), vec![52, 55, 60]); // E G C, re-centred
        assert_eq!(chord_notes(&c, 0, true, 60), vec![52, 60, 67]); // drop-2
        let slash = Chord::new(0, Quality::Maj).with_bass(Some(7));
        let n = chord_notes(&slash, 0, false, 60);
        assert_eq!(n[0] % 12, 7);
        assert!(n[0] < n[1]);
    }

    #[test]
    fn voice_leading_moves_little() {
        let c = Chord::new(0, Quality::Maj);
        let g = Chord::new(7, Quality::Maj);
        let (c_notes, _) = voice_lead(&[], &c, false, 60);
        let (g_notes, _) = voice_lead(&c_notes, &g, false, 60);
        let total: i32 = g_notes.iter().zip(&c_notes).map(|(a, b)| (*a as i32 - *b as i32).abs()).sum();
        assert!(total <= 5, "C -> G should be a small move, got {total}: {g_notes:?}");
    }

    #[test]
    fn in_key_detection() {
        assert!(Chord::new(7, Quality::Dom7).in_key(0, Scale::Major));
        assert!(Chord::new(7, Quality::Maj).in_key(9, Scale::Minor));
        assert!(!Chord::new(4, Quality::Maj).in_key(9, Scale::Minor));
        assert!(Chord::new(4, Quality::Maj).in_key(9, Scale::HarmonicMinor));
    }
}
