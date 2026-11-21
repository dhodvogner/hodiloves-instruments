//! Harmony assistant: key detection, chords that fit a melody, and "what comes next" suggestions,
//! each with a plain-language reason so the suggestions teach instead of just guessing.

use crate::project::Note;
use crate::style::Genre;
use crate::theory::*;

// ---------------------------------------------------------------------------------------------
// Key detection (Krumhansl-Schmuckler)
// ---------------------------------------------------------------------------------------------

const MAJOR_PROFILE: [f32; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE: [f32; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

#[derive(Clone, Copy, Debug)]
pub struct KeyGuess {
    pub key: u8,
    pub scale: Scale,
    /// Correlation, -1..1.
    pub score: f32,
}

/// How much each pitch class is played (length times velocity).
pub fn pc_weights(notes: &[Note]) -> [f32; 12] {
    let mut w = [0.0f32; 12];
    for n in notes {
        w[(n.pitch % 12) as usize] += n.len.max(0.05) * (0.4 + n.vel as f32 / 127.0);
    }
    w
}

fn correlation(a: &[f32; 12], profile: &[f32; 12], shift: usize) -> f32 {
    let mean_a = a.iter().sum::<f32>() / 12.0;
    let mean_p = profile.iter().sum::<f32>() / 12.0;
    let (mut num, mut da, mut dp) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        let x = a[(i + shift) % 12] - mean_a;
        let y = profile[i] - mean_p;
        num += x * y;
        da += x * x;
        dp += y * y;
    }
    if da <= 0.0 || dp <= 0.0 { 0.0 } else { num / (da.sqrt() * dp.sqrt()) }
}

/// Best matching keys, most likely first (major and natural minor only: relative keys share notes,
/// so the first guess may be the relative key of what you hear).
pub fn detect_key(weights: &[f32; 12]) -> Vec<KeyGuess> {
    let mut guesses = Vec::new();
    for key in 0..12u8 {
        guesses.push(KeyGuess { key, scale: Scale::Major, score: correlation(weights, &MAJOR_PROFILE, key as usize) });
        guesses.push(KeyGuess { key, scale: Scale::Minor, score: correlation(weights, &MINOR_PROFILE, key as usize) });
    }
    guesses.sort_by(|a, b| b.score.total_cmp(&a.score));
    guesses.truncate(5);
    guesses
}

// ---------------------------------------------------------------------------------------------
// Chords that fit a melody
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Match {
    pub chord: Chord,
    /// 0..1, higher fits better.
    pub score: f32,
    pub reason: String,
}

/// Every chord worth considering for a key: its own diatonic chords plus borrowed ones.
pub fn candidates(key: u8, scale: Scale) -> Vec<Chord> {
    let mut out: Vec<Chord> = Vec::new();
    let mut push = |c: Chord| {
        if !out.contains(&c) {
            out.push(c);
        }
    };
    for ext in [Ext::Triad, Ext::Seventh, Ext::Ninth, Ext::Sus2, Ext::Sus4, Ext::Add9] {
        for d in 0..7 {
            push(diatonic_chord(key, scale, d, ext));
        }
    }
    for other in [Scale::Major, Scale::Minor, Scale::Dorian, Scale::Mixolydian, Scale::HarmonicMinor, Scale::Phrygian, Scale::Lydian] {
        if other == scale {
            continue;
        }
        for ext in [Ext::Triad, Ext::Seventh] {
            for d in 0..7 {
                push(diatonic_chord(key, other, d, ext));
            }
        }
    }
    out
}

fn list_pcs(pcs: &[u8], flats: bool) -> String {
    pcs.iter().map(|&p| pc_name(p, flats)).collect::<Vec<_>>().join(", ")
}

/// Rank chords by how well they fit a set of melody notes (given as pitch-class weights).
pub fn match_melody(weights: &[f32; 12], key: u8, scale: Scale, genre: Genre) -> Vec<Match> {
    let total: f32 = weights.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let flats = use_flats(key, scale);
    let played: Vec<u8> = (0..12u8).filter(|&p| weights[p as usize] > 0.0).collect();
    let mut out: Vec<Match> = candidates(key, scale)
        .into_iter()
        .map(|chord| {
            let pcs = chord.pitch_classes();
            let covered: f32 = pcs.iter().map(|&p| weights[p as usize]).sum();
            // A melody note a half step above a chord tone grinds against it (a minor 9th).
            let clash: f32 = (0..12u8)
                .filter(|&p| !pcs.contains(&p) && pcs.contains(&((p + 11) % 12)))
                .map(|p| weights[p as usize])
                .sum();
            let root_bonus = if weights[chord.root as usize] > 0.0 { 0.05 } else { 0.0 };
            let key_bonus = if chord.in_key(key, scale) { 0.08 } else { -0.04 };
            let simple = -0.02 * (chord.intervals().len() as f32 - 3.0).max(0.0);
            let genre_bonus = match (genre, chord.quality) {
                (Genre::Synthwave | Genre::Trance | Genre::Pop, Quality::Maj | Quality::Min) => 0.03,
                (Genre::DrumAndBass | Genre::Liquid | Genre::LoFi, Quality::Min7 | Quality::Maj7 | Quality::Min9 | Quality::Maj9) => 0.03,
                _ => 0.0,
            };
            let score = (covered / total - 0.35 * clash / total + root_bonus + key_bonus + simple + genre_bonus).clamp(0.0, 1.0);

            let inside: Vec<u8> = played.iter().copied().filter(|p| pcs.contains(p)).collect();
            let outside: Vec<u8> = played.iter().copied().filter(|p| !pcs.contains(p)).collect();
            let mut reason = if inside.is_empty() {
                "Shares no notes with the selection".to_string()
            } else {
                format!("Contains {} from the melody", list_pcs(&inside, flats))
            };
            if !outside.is_empty() {
                reason.push_str(&format!("; {} pass over it as non-chord tones", list_pcs(&outside, flats)));
            }
            if !chord.in_key(key, scale) {
                reason.push_str(". Borrowed: not fully in the key");
            }
            Match { chord, score, reason }
        })
        .collect();
    out.sort_by(|a, b| b.score.total_cmp(&a.score));
    out
}

// ---------------------------------------------------------------------------------------------
// What comes next
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub chord: Chord,
    pub score: f32,
    pub reason: String,
}

/// Scale degrees (0-based) favoured as a move from one chord to the next, per genre.
fn genre_moves(genre: Genre, minor: bool) -> &'static [(usize, usize)] {
    match (genre, minor) {
        (Genre::Synthwave, true) => &[(0, 5), (5, 2), (2, 6), (6, 0), (0, 6), (6, 5), (5, 6), (3, 4), (0, 2), (2, 5)],
        (Genre::Synthwave, false) => &[(3, 4), (4, 2), (2, 5), (5, 3), (0, 4), (4, 5), (5, 3), (3, 0)],
        (Genre::DrumAndBass, true) | (Genre::Liquid, true) => &[(0, 6), (6, 5), (5, 6), (0, 5), (5, 0), (6, 0), (0, 3), (3, 6), (5, 3)],
        (Genre::Liquid, false) => &[(0, 2), (2, 1), (1, 4), (4, 0), (0, 5), (5, 1)],
        (Genre::Pop, false) => &[(0, 4), (4, 5), (5, 3), (3, 0), (0, 5), (5, 3), (3, 4)],
        (Genre::Pop, true) => &[(0, 5), (5, 2), (2, 6), (6, 0), (0, 3)],
        (Genre::Cinematic, true) => &[(5, 6), (6, 0), (0, 5), (5, 3), (3, 6)],
        (Genre::Cinematic, false) => &[(0, 1), (1, 0), (0, 4), (4, 5)],
        (Genre::LoFi, _) => &[(1, 4), (4, 0), (0, 5), (5, 1), (2, 5), (3, 2)],
        (Genre::Trance, true) => &[(5, 2), (2, 6), (6, 0), (0, 5), (0, 6)],
        (Genre::Trance, false) => &[(0, 6), (6, 3), (3, 0), (0, 4), (5, 3)],
        _ => &[],
    }
}

fn function_flow(from: Function, to: Function) -> f32 {
    use Function::*;
    match (from, to) {
        (Dominant, Tonic) => 1.0,
        (Predominant, Dominant) => 0.9,
        (Tonic, Predominant) => 0.6,
        (Tonic, Dominant) => 0.35,
        (Tonic, Tonic) => 0.4,
        (Predominant, Tonic) => 0.45,
        (Predominant, Predominant) => 0.3,
        (Dominant, Predominant) => 0.25,
        (Dominant, Dominant) => 0.3,
        (_, Colour) | (Colour, _) => 0.45,
    }
}

fn motion_strength(semitones_up: u8) -> (f32, &'static str) {
    match semitones_up % 12 {
        5 => (1.0, "the root falls a fifth (rises a fourth): the strongest, most natural motion in tonal music"),
        7 => (0.7, "the root rises a fifth: bright and open"),
        2 => (0.6, "the root steps up a tone: forward momentum"),
        10 => (0.65, "the root steps down a tone: a gentle slide"),
        3 => (0.55, "the root rises a minor third: shares two notes with the last chord"),
        9 => (0.6, "the root falls a minor third: shares two notes, a smooth change"),
        4 => (0.45, "the root rises a major third: a bright lift"),
        8 => (0.5, "the root falls a major third: a warm shift"),
        1 | 11 => (0.3, "the root moves by a half step: chromatic and dramatic"),
        _ => (0.15, "the root jumps a tritone: maximum surprise"),
    }
}

fn common_tones(a: &Chord, b: &Chord) -> usize {
    let pa = a.pitch_classes();
    b.pitch_classes().iter().filter(|p| pa.contains(p)).count()
}

fn borrowed_from(chord: &Chord, key: u8) -> Option<&'static str> {
    [Scale::Minor, Scale::Major, Scale::Dorian, Scale::Mixolydian, Scale::HarmonicMinor, Scale::Phrygian, Scale::Lydian]
        .iter()
        .find(|s| chord.in_key(key, **s))
        .map(|s| s.name())
}

/// Chords that could follow `prev`, best first. With no previous chord, good places to start.
pub fn suggest_next(prev: Option<&Chord>, key: u8, scale: Scale, genre: Genre) -> Vec<Suggestion> {
    let minor = scale.is_minor_like();
    let mut pool: Vec<Chord> = Vec::new();
    for ext in [Ext::Triad, Ext::Seventh] {
        for d in 0..7 {
            pool.push(diatonic_chord(key, scale, d, ext));
        }
    }
    // A few borrowed colours: the usual suspects of modal interchange.
    for other in [Scale::Minor, Scale::Major, Scale::Mixolydian, Scale::Dorian, Scale::HarmonicMinor] {
        if other != scale {
            for d in 0..7 {
                let c = diatonic_chord(key, other, d, Ext::Triad);
                if !c.in_key(key, scale) && !pool.contains(&c) {
                    pool.push(c);
                }
            }
        }
    }

    let mut out: Vec<Suggestion> = Vec::new();
    for chord in pool {
        let in_key = chord.in_key(key, scale);
        let func = function_of(key, scale, &chord);
        let deg = degree_of(key, scale, chord.root);

        let (score, reason) = match prev {
            None => {
                let start = match deg {
                    Some(0) => 1.0,
                    Some(5) | Some(2) => 0.6,
                    Some(3) => 0.5,
                    Some(4) => 0.3,
                    _ => 0.15,
                };
                let why = if deg == Some(0) {
                    "Starts on the tonic: the home chord of the key".to_string()
                } else {
                    format!("{} chord: {}", func.label(), func.explain())
                };
                (start * if in_key { 1.0 } else { 0.6 }, why)
            }
            Some(p) => {
                if p.root == chord.root && p.quality == chord.quality {
                    continue;
                }
                let up = ((chord.root as i32 - p.root as i32).rem_euclid(12)) as u8;
                let (motion, motion_text) = motion_strength(up);
                let pf = function_of(key, scale, p);
                let flow = function_flow(pf, func);
                let common = common_tones(p, &chord);
                let mut score = 0.45 * motion + 0.3 * flow + 0.06 * common as f32;
                let mut bits: Vec<String> = Vec::new();
                bits.push(format!("{}{}", motion_text[..1].to_uppercase(), &motion_text[1..]));
                if common > 0 {
                    bits.push(format!("{common} note{} in common", if common == 1 { "" } else { "s" }));
                }
                match (pf, func) {
                    (Function::Dominant, Function::Tonic) => bits.push("resolves tension back home".into()),
                    (Function::Predominant, Function::Dominant) => bits.push("builds tension towards a resolution".into()),
                    (Function::Tonic, Function::Predominant) => bits.push("leaves home and builds anticipation".into()),
                    (_, Function::Colour) => {}
                    _ => {}
                }
                if let (Some(a), Some(b)) = (degree_of(key, scale, p.root), deg) {
                    if genre_moves(genre, minor).contains(&(a, b)) {
                        score += 0.3;
                        bits.push(format!("a typical {} move", genre.name()));
                    }
                }
                if !in_key {
                    score -= 0.12;
                    let from = borrowed_from(&chord, key).unwrap_or("another mode");
                    bits.push(format!("borrowed from {from}: a colour outside the key"));
                }
                (score, bits.join("; "))
            }
        };
        out.push(Suggestion { chord, score, reason });
    }
    // Prefer the plain triad when a triad and its seventh score the same.
    out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.chord.intervals().len().cmp(&b.chord.intervals().len())));
    let max = out.first().map(|s| s.score).unwrap_or(1.0).max(0.01);
    for s in &mut out {
        s.score = (s.score / max).clamp(0.0, 1.0);
    }
    out.truncate(12);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(pitches: &[u8]) -> Vec<Note> {
        pitches.iter().enumerate().map(|(i, &p)| Note::new(i as f32, 1.0, p, 100)).collect()
    }

    #[test]
    fn detects_a_minor_from_its_melody() {
        // A minor melody that leans on the tonic and fifth
        let n = notes(&[69, 72, 76, 74, 72, 71, 69, 67, 69, 64, 69, 72, 69]);
        let guesses = detect_key(&pc_weights(&n));
        let best = guesses[0];
        assert_eq!((best.key, best.scale), (9, Scale::Minor), "{guesses:?}");
    }

    #[test]
    fn detects_c_major() {
        let n = notes(&[60, 62, 64, 65, 67, 69, 71, 72, 67, 64, 60, 67, 60]);
        let best = detect_key(&pc_weights(&n))[0];
        assert_eq!((best.key, best.scale), (0, Scale::Major));
    }

    #[test]
    fn melody_a_c_e_matches_a_minor_first() {
        let w = pc_weights(&notes(&[69, 72, 76]));
        let m = match_melody(&w, 9, Scale::Minor, Genre::Any);
        assert_eq!(m[0].chord.name(false), "Am", "{:?}", m.iter().take(4).map(|x| x.chord.name(false)).collect::<Vec<_>>());
        assert!(m[0].score > 0.9);
        assert!(m[0].reason.contains("C, E, A"));
    }

    #[test]
    fn melody_with_a_clash_avoids_the_clashing_chord() {
        // F over a C major chord clashes with E (a half step); F major is better.
        let w = pc_weights(&notes(&[65, 69, 72]));
        let m = match_melody(&w, 0, Scale::Major, Genre::Any);
        assert_eq!(m[0].chord.name(false), "F");
    }

    #[test]
    fn after_g7_in_c_major_the_tonic_is_suggested() {
        let g7 = diatonic_chord(0, Scale::Major, 4, Ext::Seventh);
        let s = suggest_next(Some(&g7), 0, Scale::Major, Genre::Any);
        let top: Vec<_> = s.iter().take(3).map(|x| x.chord.name(false)).collect();
        assert!(top.iter().any(|n| n == "C"), "{top:?}");
        assert!(s[0].reason.contains("fifth"), "{}", s[0].reason);
    }

    #[test]
    fn synthwave_prefers_its_loop() {
        let am = diatonic_chord(9, Scale::Minor, 0, Ext::Triad);
        let s = suggest_next(Some(&am), 9, Scale::Minor, Genre::Synthwave);
        let top: Vec<_> = s.iter().take(3).map(|x| x.chord.name(false)).collect();
        assert!(top.contains(&"F".to_string()) || top.contains(&"G".to_string()), "{top:?}");
    }

    #[test]
    fn first_chord_suggestion_is_the_tonic() {
        let s = suggest_next(None, 9, Scale::Minor, Genre::Any);
        assert_eq!(s[0].chord.name(false), "Am");
    }
}
