//! A drum part for the sketch, shaped for Neon Break (but plain General MIDI drum notes work anywhere).
//!
//! Two flavours:
//! - **Pattern keys**: Neon Break is played with keys: key 72 and up plays pattern slot 1..12 for as long as
//!   the note lasts, and keys 60 / 62 / 64 / 65 play fills. Each section becomes one long pattern note,
//!   with a fill note over its last bar.
//! - **Notes**: the same patterns written out as ordinary drum notes (36 kick, 38 snare, 39 clap, 42 / 46
//!   hats, 50 / 45 toms, 49 cymbal) so any drum machine or sampler can play them.
//!
//! The pattern rows use Neon Break's notation, one row per drum (kick, snare, clap, closed hat, open hat,
//! high tom, low tom, cymbal) and 16 steps per bar: `.` rest, `g` ghost, `x` soft, `X` normal, `!` accent.

use crate::project::*;
use crate::style::Genre;

/// Key of pattern slot 1 in Neon Break.
pub const PATTERN_KEY: u8 = 72;
/// Drum notes for the eight rows: kick, snare, clap, closed hat, open hat, high tom, low tom, cymbal.
pub const DRUM_NOTES: [u8; 8] = [36, 38, 39, 42, 46, 50, 45, 49];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrumMode {
    PatternKeys,
    Notes,
}

pub struct DrumPattern {
    pub name: &'static str,
    pub blurb: &'static str,
    /// Slot (0-based) this pattern has in Neon Break's default bank.
    pub slot: u8,
    pub rows: [&'static str; 8],
}

const NONE: &str = "................";

/// The patterns of Neon Break's default bank that Chordwave uses.
pub const PATTERNS: [DrumPattern; 8] = [
    DrumPattern { name: "Two-Step", blurb: "Classic drum and bass: kick on 1 and the 'and' of 2, snare on 2 and 4, lots of space.", slot: 0, rows: ["X.........X.....", "....X.......X...", NONE, "x.x.x.x.x.x.x.x.", NONE, NONE, NONE, NONE] },
    DrumPattern { name: "Roller", blurb: "Rolling hats and ghost snares keep the energy flowing without big gaps.", slot: 1, rows: ["X.........X.....", "....X..g.g..X..g", NONE, "x.xxx.xxx.xxx.xx", "......x.......x.", NONE, NONE, NONE] },
    DrumPattern { name: "Dancefloor", blurb: "Extra kick and a clap on the backbeat: a club-friendly drum and bass groove.", slot: 4, rows: ["X.....X...X.....", "....X.......X...", "....X.......X...", "x.x.x.x.x.x.x.x.", "..............x.", NONE, NONE, NONE] },
    DrumPattern { name: "Liquid", blurb: "Soft and smooth: gentle hats and a cymbal shimmer. Pairs with 9th chords.", slot: 5, rows: ["X.........X.....", "....X.......X...", NONE, "x.xgx.xgx.xgx.xg", NONE, NONE, NONE, "x...x...x...x..."] },
    DrumPattern { name: "Neon Chase", blurb: "Busy and bright: accents, toms and open hats. Good for a chorus.", slot: 8, rows: ["X.....X...X.....", "....!.......!...", "....X.......X...", "x.xxx.xxx.xxx.xx", NONE, ".......x........", "..............xX", NONE] },
    DrumPattern { name: "Half Backbeat", blurb: "Slow, steady backbeat with a clap: a synthwave and pop basic.", slot: 9, rows: ["X.......X.X.....", "....X.......X...", "....X.......X...", "x.x.x.x.x.x.x.x.", "..............x.", NONE, NONE, NONE] },
    DrumPattern { name: "Outrun", blurb: "Driving four-on-the-floor kick with a big snare: the classic synthwave drum sound.", slot: 10, rows: ["X...X...X...X...", "....!.......!...", "....X.......X...", "x.x.x.x.x.x.x.x.", "..x...x...x...x.", NONE, NONE, NONE] },
    DrumPattern { name: "Breakdown", blurb: "Sparse: just a few hits. For intros, breakdowns and quiet sections.", slot: 11, rows: ["X...............", "........X.......", NONE, "x...x...x...x...", NONE, NONE, NONE, "!..............."] },
];

fn pattern_by_slot(slot: u8) -> &'static DrumPattern {
    PATTERNS.iter().find(|p| p.slot == slot).unwrap_or(&PATTERNS[5])
}

/// (main, second) pattern slots per genre: sections alternate between the two.
fn slots_for(genre: Genre) -> (u8, u8) {
    match genre {
        Genre::Any | Genre::Pop => (9, 4),
        Genre::Synthwave => (10, 8),
        Genre::DrumAndBass => (0, 1),
        Genre::Liquid => (5, 0),
        Genre::Cinematic => (11, 9),
        Genre::LoFi => (9, 11),
        Genre::Trance => (4, 10),
    }
}

/// The Neon Break fill key played over the last bar of a section.
pub fn fill_key(genre: Genre) -> u8 {
    match genre {
        Genre::Synthwave => 62,                      // tom fill
        Genre::Trance | Genre::Cinematic => 65,      // build
        Genre::LoFi => 64,                           // hat roll
        _ => 60,                                     // snare rush
    }
}

/// The pattern for a section: the user's choice (an index into `PATTERNS`), or the genre's own
/// pair of patterns, alternating between sections.
pub fn pattern_for(genre: Genre, section: usize, choice: Option<usize>) -> &'static DrumPattern {
    if let Some(i) = choice {
        return &PATTERNS[i.min(PATTERNS.len() - 1)];
    }
    let (a, b) = slots_for(genre);
    pattern_by_slot(if section % 2 == 0 { a } else { b })
}

fn velocity(ch: u8) -> Option<u8> {
    match ch {
        b'g' => Some(38),
        b'x' => Some(76),
        b'X' => Some(108),
        b'!' => Some(127),
        b'?' => Some(90),
        _ => None, // '.' rest, 'o' (a ghost that only plays half the time) is left out of a fixed export
    }
}

/// Render the drum part for the whole sketch.
pub fn render_drums(project: &Project, genre: Genre, mode: DrumMode, choice: Option<usize>, fills: bool) -> Vec<Note> {
    let mut out = Vec::new();
    let mut start = 0.0f32;
    for (si, section) in project.sections.iter().enumerate() {
        let beats = section.beats();
        if beats <= 0.0 {
            continue;
        }
        let pattern = pattern_for(genre, si, choice);
        let bars = (beats / BEATS_PER_BAR).ceil().max(1.0) as usize;
        let fill = fills && bars >= 2;
        match mode {
            DrumMode::PatternKeys => {
                let main_len = if fill { beats - BEATS_PER_BAR } else { beats };
                out.push(Note::new(start, main_len, PATTERN_KEY + pattern.slot, 100));
                if fill {
                    out.push(Note::new(start + main_len, BEATS_PER_BAR, fill_key(genre), 100));
                }
            }
            DrumMode::Notes => {
                for bar in 0..bars {
                    let bar_start = start + bar as f32 * BEATS_PER_BAR;
                    let last_fill_bar = fill && bar + 1 == bars;
                    for (row, text) in pattern.rows.iter().enumerate() {
                        for (step, ch) in text.bytes().enumerate() {
                            // The last beat of a fill bar is replaced by a snare roll below.
                            if last_fill_bar && step >= 12 {
                                continue;
                            }
                            let t = bar_start + step as f32 * 0.25;
                            if t >= start + beats - 0.001 {
                                continue;
                            }
                            if let Some(v) = velocity(ch) {
                                out.push(Note::new(t, 0.2, DRUM_NOTES[row], v));
                            }
                        }
                    }
                    if last_fill_bar {
                        for (i, v) in [70u8, 84, 100, 122].iter().enumerate() {
                            out.push(Note::new(bar_start + 3.0 + i as f32 * 0.25, 0.2, DRUM_NOTES[1], *v));
                        }
                    }
                }
                // A crash on the first beat of every section after the first.
                if si > 0 {
                    out.push(Note::new(start, 1.0, DRUM_NOTES[7], 104));
                }
            }
        }
        start += beats;
    }
    out.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sketch(sections: &[f32]) -> Project {
        let mut p = Project::default();
        for (i, beats) in sections.iter().enumerate() {
            p.sections.push(Section { name: format!("{i}"), items: vec![ChordItem::new(crate::theory::Chord::new(9, crate::theory::Quality::Min), *beats)] });
        }
        p
    }

    #[test]
    fn every_pattern_row_is_sixteen_steps_and_slots_are_unique() {
        let mut slots = std::collections::HashSet::new();
        for p in &PATTERNS {
            assert!(slots.insert(p.slot), "{}", p.name);
            for row in p.rows {
                assert_eq!(row.len(), 16, "{}", p.name);
            }
        }
        for g in crate::style::GENRES {
            let (a, b) = slots_for(g);
            assert!(PATTERNS.iter().any(|p| p.slot == a) && PATTERNS.iter().any(|p| p.slot == b), "{g:?}");
        }
    }

    #[test]
    fn pattern_keys_play_the_slot_and_end_with_a_fill() {
        let p = sketch(&[16.0, 8.0]);
        let n = render_drums(&p, Genre::Synthwave, DrumMode::PatternKeys, None, true);
        // section A: Outrun (slot 11 -> key 82) for 3 bars then a tom fill (62) over the last bar
        assert_eq!(n[0], Note::new(0.0, 12.0, 82, 100));
        assert_eq!(n[1], Note::new(12.0, 4.0, 62, 100));
        // section B: Neon Chase (slot 9 -> key 80), 1 bar pattern + fill
        assert_eq!(n[2], Note::new(16.0, 4.0, 80, 100));
        assert_eq!(n[3], Note::new(20.0, 4.0, 62, 100));
        assert_eq!(n.len(), 4);
    }

    #[test]
    fn a_one_bar_section_has_no_fill() {
        let n = render_drums(&sketch(&[4.0]), Genre::DrumAndBass, DrumMode::PatternKeys, None, true);
        assert_eq!(n, vec![Note::new(0.0, 4.0, 72, 100)]); // Two-Step = slot 1
    }

    #[test]
    fn drum_notes_follow_the_pattern() {
        let n = render_drums(&sketch(&[8.0]), Genre::DrumAndBass, DrumMode::Notes, None, true);
        // Two-Step bar 1: kick on step 0 and 10, snare on step 4 and 12; bar 2 ends in a snare roll
        let kicks: Vec<f32> = n.iter().filter(|x| x.pitch == 36).map(|x| x.start).collect();
        assert_eq!(kicks, vec![0.0, 2.5, 4.0, 6.5]);
        let snares: Vec<f32> = n.iter().filter(|x| x.pitch == 38).map(|x| x.start).collect();
        assert_eq!(snares, vec![1.0, 3.0, 5.0, 7.0, 7.25, 7.5, 7.75]);
        assert!(n.iter().all(|x| x.end() <= 8.001));
        // no hats after the fill starts
        assert!(n.iter().filter(|x| x.pitch == 42).all(|x| x.start < 7.0));
    }

    #[test]
    fn later_sections_start_with_a_crash() {
        let n = render_drums(&sketch(&[8.0, 8.0]), Genre::Synthwave, DrumMode::Notes, None, true);
        assert!(n.iter().any(|x| x.pitch == 49 && x.start == 8.0));
        assert!(!n.iter().any(|x| x.pitch == 49 && x.start == 0.0));
    }

    #[test]
    fn a_chosen_pattern_and_the_fills_switch_are_respected() {
        let p = sketch(&[16.0, 8.0]);
        // choice 6 = Outrun (slot 11 -> key 82) in every section, no fills
        let n = render_drums(&p, Genre::DrumAndBass, DrumMode::PatternKeys, Some(6), false);
        assert_eq!(n, vec![Note::new(0.0, 16.0, 82, 100), Note::new(16.0, 8.0, 82, 100)]);
        let n = render_drums(&p, Genre::DrumAndBass, DrumMode::Notes, Some(6), false);
        assert!(n.iter().all(|x| x.start < 24.0));
        // no snare roll without fills
        assert!(!n.iter().any(|x| x.pitch == 38 && (x.start - 15.25).abs() < 0.01));
    }

    #[test]
    fn empty_sketch_makes_no_drums() {
        assert!(render_drums(&Project::default(), Genre::Any, DrumMode::Notes, None, true).is_empty());
    }
}
