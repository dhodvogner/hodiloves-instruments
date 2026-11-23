//! Melody generator: writes a hook over the chord progression.
//!
//! The rules follow common practice: notes on strong beats are chord tones, notes in between move
//! by step through the scale, the line is built from a repeated rhythmic motif with variation,
//! leaps are rare, and each chord ends on a chord tone so the line always sounds "right".

use crate::project::*;
use crate::style::Genre;
use crate::theory::*;
use plugin_common::dsp::Rng;

#[derive(Clone, Copy, Debug)]
pub struct MelodyCfg {
    pub key: u8,
    pub scale: Scale,
    pub genre: Genre,
    pub seed: u32,
    /// 0 = sparse and long notes, 1 = busy.
    pub density: f32,
    pub lo: i32,
    pub hi: i32,
}

type Rhythm = &'static [(u8, u8)]; // (start in 16ths, length in 16ths) within one bar

fn rhythms(genre: Genre) -> &'static [Rhythm] {
    match genre {
        Genre::Synthwave => &[
            &[(0, 16)],
            &[(0, 8), (8, 4), (12, 4)],
            &[(0, 6), (6, 2), (8, 4), (12, 4)],
            &[(0, 4), (4, 4), (8, 6), (14, 2)],
            &[(0, 2), (2, 2), (4, 2), (6, 2), (8, 2), (10, 2), (12, 4)],
        ],
        Genre::DrumAndBass | Genre::Liquid => &[
            &[(0, 4), (8, 4)],
            &[(2, 2), (6, 2), (8, 4)],
            &[(0, 3), (6, 2), (10, 3), (14, 2)],
            &[(0, 2), (3, 3), (8, 2), (11, 5)],
            &[(0, 2), (4, 2), (7, 2), (10, 2), (14, 2)],
        ],
        Genre::Cinematic => &[&[(0, 16)], &[(0, 8), (8, 8)], &[(0, 12), (12, 4)], &[(0, 6), (6, 2), (8, 8)]],
        Genre::Trance => &[
            &[(0, 8), (8, 8)],
            &[(0, 4), (4, 4), (8, 4), (12, 4)],
            &[(0, 2), (2, 2), (4, 2), (6, 2), (8, 2), (10, 2), (12, 2), (14, 2)],
        ],
        _ => &[
            &[(0, 8), (8, 8)],
            &[(0, 6), (6, 2), (8, 8)],
            &[(0, 4), (4, 4), (8, 4), (12, 4)],
            &[(0, 4), (4, 4), (8, 2), (10, 2), (12, 4)],
            &[(0, 2), (2, 2), (4, 4), (8, 4), (12, 2), (14, 2)],
        ],
    }
}

fn pick_rhythm(genre: Genre, density: f32, rng: &mut Rng) -> Rhythm {
    let list = rhythms(genre);
    // Pools are ordered from calm to busy: density chooses a window of them.
    let centre = density.clamp(0.0, 1.0) * (list.len() - 1) as f32;
    let i = (centre + rng.next_bipolar() * 1.2).round().clamp(0.0, (list.len() - 1) as f32) as usize;
    list[i]
}

pub fn generate_melody(project: &Project, cfg: &MelodyCfg) -> Vec<Note> {
    let mut rng = Rng::new(cfg.seed.wrapping_mul(2654435761).wrapping_add(12345));
    let scale_pool: Vec<i32> = scale_notes(cfg.key, cfg.scale, cfg.lo, cfg.hi).into_iter().map(|n| n as i32).collect();
    let placed = project.placed();
    if scale_pool.is_empty() || placed.is_empty() {
        return Vec::new();
    }
    let total = project.beats();

    // Two rhythms (A and B) alternate bar by bar, like a verse of question and answer.
    let rhythm_a = pick_rhythm(cfg.genre, cfg.density, &mut rng);
    let rhythm_b = pick_rhythm(cfg.genre, cfg.density, &mut rng);
    // One motif of stepwise moves, reused on every bar with small variations.
    let motif: Vec<i32> = (0..16)
        .map(|_| match rng.next_u32() % 10 {
            0 | 1 => -2,
            2..=3 => -1,
            4 => 0,
            5..=7 => 1,
            8 => 2,
            _ => -1,
        })
        .collect();

    let mid = (cfg.lo + cfg.hi) / 2;
    let mut idx = scale_pool.iter().enumerate().min_by_key(|(_, n)| (**n - mid).abs()).map(|(i, _)| i).unwrap_or(0);
    let mut out: Vec<Note> = Vec::new();
    let mut note_counter = 0usize;

    for (ci, p) in placed.iter().enumerate() {
        let (c_start, c_end) = (p.start, p.start + p.item_beats);
        let chord_pcs = p.chord.pitch_classes();
        let is_last_chord = ci + 1 == placed.len();
        let mut bar = 0usize;
        let mut chord_notes_out: Vec<Note> = Vec::new();
        while c_start + (bar as f32) * BEATS_PER_BAR < c_end - 0.001 {
            let global_bar = ((c_start + bar as f32 * BEATS_PER_BAR) / BEATS_PER_BAR).floor() as usize;
            let rhythm = if global_bar % 2 == 0 { rhythm_a } else { rhythm_b };
            for (ri, &(s16, l16)) in rhythm.iter().enumerate() {
                let t = c_start + bar as f32 * BEATS_PER_BAR + s16 as f32 * 0.25;
                if t >= c_end - 0.001 {
                    break;
                }
                // Thin out busy rhythms when density is low (never the first note of a bar).
                if ri > 0 && cfg.density < 0.5 && rng.next_f32() < (0.5 - cfg.density) * 0.8 {
                    continue;
                }
                let len = (l16 as f32 * 0.25).min(c_end - t);
                let strong = s16 % 4 == 0 && (s16 % 8 == 0 || ri == 0);

                // Contour: rise through the first half of the sketch, fall through the second.
                let progress = if total > 0.0 { t / total } else { 0.0 };
                let bias = if progress < 0.5 { 0.35 } else { -0.35 };
                let mut step = motif[(note_counter + global_bar) % motif.len()];
                if rng.next_f32() < 0.25 {
                    step = if rng.next_f32() < 0.5 + bias { 1 } else { -1 };
                }
                let mut target = (idx as i32 + step).clamp(0, scale_pool.len() as i32 - 1) as usize;

                // A pitch that would be the third repeat in a row is not allowed.
                let forbidden = match out.as_slice() {
                    [.., a, b] if a.pitch == b.pitch => Some(b.pitch as i32),
                    _ => None,
                };
                if strong {
                    // Land on the closest chord tone around the target.
                    let mut best: Option<(i32, usize)> = None;
                    for d in -4i32..=4 {
                        let j = target as i32 + d;
                        if j < 0 || j >= scale_pool.len() as i32 || Some(scale_pool[j as usize]) == forbidden {
                            continue;
                        }
                        let pc = (scale_pool[j as usize].rem_euclid(12)) as u8;
                        if chord_pcs.contains(&pc) {
                            let cost = d.abs() * 2 + if j as usize == idx { 1 } else { 0 };
                            if best.is_none_or(|b| cost < b.0) {
                                best = Some((cost, j as usize));
                            }
                        }
                    }
                    if let Some((_, j)) = best {
                        target = j;
                    }
                } else if Some(scale_pool[target]) == forbidden {
                    target = if target + 1 < scale_pool.len() { target + 1 } else { target.saturating_sub(1) };
                }
                idx = target;
                let vel = (if strong { 100 } else { 84 } + (rng.next_bipolar() * 8.0) as i32).clamp(40, 127) as u8;
                chord_notes_out.push(Note::new(t, len, scale_pool[idx] as u8, vel));
                note_counter += 1;
            }
            bar += 1;
        }
        // The last note of a chord must be a chord tone; the very last note of the sketch is the root.
        if let Some(last) = chord_notes_out.last_mut() {
            let want_root = is_last_chord;
            let current = last.pitch as i32;
            let best = scale_pool
                .iter()
                .copied()
                .filter(|n| {
                    let pc = n.rem_euclid(12) as u8;
                    if want_root { pc == p.chord.root } else { chord_pcs.contains(&pc) }
                })
                .min_by_key(|n| (*n - current).abs());
            if let Some(n) = best {
                last.pitch = n as u8;
                idx = scale_pool.iter().position(|x| *x == n).unwrap_or(idx);
            }
        }
        out.extend(chord_notes_out);
    }
    out
}


// ---------------------------------------------------------------------------------------------
// Styles: different ways to write a melody
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MelodyStyle {
    Hook,
    Riff,
    Arp,
    Long,
    CallResponse,
    Repitch,
}

pub const MELODY_STYLES: [MelodyStyle; 6] =
    [MelodyStyle::Hook, MelodyStyle::Riff, MelodyStyle::Arp, MelodyStyle::Long, MelodyStyle::CallResponse, MelodyStyle::Repitch];

impl MelodyStyle {
    pub fn name(self) -> &'static str {
        match self {
            MelodyStyle::Hook => "Hook",
            MelodyStyle::Riff => "Riff",
            MelodyStyle::Arp => "Arpeggio",
            MelodyStyle::Long => "Long notes",
            MelodyStyle::CallResponse => "Call & answer",
            MelodyStyle::Repitch => "Re-pitch mine",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            MelodyStyle::Hook => "A singable tune: chord tones on the strong beats, steps in between, a rhythm that alternates bar by bar. Different every time you press it.",
            MelodyStyle::Riff => "One short pattern, repeated on every chord and moved to follow it. Very consistent and catchy: the classic synthwave and drum and bass riff.",
            MelodyStyle::Arp => "Plays the notes of each chord one after another, up and back down. Predictable, flowing and always in tune with the chords.",
            MelodyStyle::Long => "Slow, sustained notes: one or two per bar, each a chord tone. Calm and emotional, good under a vocal or for pads and cinematic parts.",
            MelodyStyle::CallResponse => "A busy phrase in one bar, then a short answer with a long final note in the next. Feels like a conversation.",
            MelodyStyle::Repitch => "Keeps the rhythm of your notes and picks new pitches for them. Use it after drawing a rhythm you like.",
        }
    }
}

/// Write a melody in the chosen style. `existing` is the current melody (used by `Repitch`).
pub fn generate(project: &Project, cfg: &MelodyCfg, style: MelodyStyle, existing: &[Note]) -> Vec<Note> {
    match style {
        MelodyStyle::Hook => generate_melody(project, cfg),
        MelodyStyle::Riff => riff(project, cfg),
        MelodyStyle::Arp => arpeggio(project, cfg),
        MelodyStyle::Long => long_notes(project, cfg),
        MelodyStyle::CallResponse => call_response(project, cfg),
        MelodyStyle::Repitch => repitch(project, cfg, existing),
    }
}

fn pool(cfg: &MelodyCfg) -> Vec<i32> {
    scale_notes(cfg.key, cfg.scale, cfg.lo, cfg.hi).into_iter().map(|n| n as i32).collect()
}

/// Index into `pool` of the note with pitch class `pc` closest to `target`.
fn nearest_pc(pool: &[i32], pc: u8, target: i32) -> usize {
    pool.iter().enumerate().filter(|(_, n)| n.rem_euclid(12) as u8 == pc).min_by_key(|(_, n)| (**n - target).abs()).map(|(i, _)| i).unwrap_or(0)
}

fn riff(project: &Project, cfg: &MelodyCfg) -> Vec<Note> {
    let mut rng = Rng::new(cfg.seed.wrapping_mul(2654435761).wrapping_add(777));
    let pool = pool(cfg);
    let placed = project.placed();
    if pool.is_empty() || placed.is_empty() {
        return Vec::new();
    }
    let rhythm = pick_rhythm(cfg.genre, cfg.density, &mut rng);
    // The pattern as steps through the scale, relative to the chord root. It starts on the root.
    let mut motif = vec![0i32];
    for _ in 1..rhythm.len() {
        let prev = *motif.last().unwrap();
        let step = match rng.next_u32() % 8 {
            0 | 1 => -1,
            2 | 3 => 1,
            4 => 2,
            5 => -2,
            6 => 0,
            _ => 3,
        };
        motif.push((prev + step).clamp(-3, 4));
    }
    let mid = (cfg.lo + cfg.hi) / 2;
    let mut out = Vec::new();
    for p in &placed {
        let end = p.start + p.item_beats;
        let base = nearest_pc(&pool, p.chord.root, mid) as i32;
        let mut bar = 0.0;
        while p.start + bar < end - 0.001 {
            for (k, &(s16, l16)) in rhythm.iter().enumerate() {
                let t = p.start + bar + s16 as f32 * 0.25;
                if t >= end - 0.001 {
                    break;
                }
                let idx = (base + motif[k]).clamp(0, pool.len() as i32 - 1) as usize;
                let vel = if s16 % 4 == 0 { 100 } else { 86 };
                out.push(Note::new(t, (l16 as f32 * 0.25).min(end - t), pool[idx] as u8, vel));
            }
            bar += BEATS_PER_BAR;
        }
    }
    out
}

fn arpeggio(project: &Project, cfg: &MelodyCfg) -> Vec<Note> {
    let pool = pool(cfg);
    let placed = project.placed();
    if pool.is_empty() || placed.is_empty() {
        return Vec::new();
    }
    let step = if cfg.density < 0.34 { 1.0 } else if cfg.density < 0.67 { 0.5 } else { 0.25 };
    let mid = (cfg.lo + cfg.hi) / 2;
    let mut out = Vec::new();
    for p in &placed {
        let end = p.start + p.item_beats;
        let pcs = p.chord.pitch_classes();
        let tones: Vec<i32> = pool.iter().copied().filter(|n| pcs.contains(&(n.rem_euclid(12) as u8))).collect();
        if tones.is_empty() {
            continue;
        }
        // Four chord tones around the middle of the range, going up and then back down.
        let start_i = tones.iter().position(|n| *n >= mid - 4).unwrap_or(0).min(tones.len().saturating_sub(1));
        let up: Vec<i32> = tones.iter().copied().skip(start_i).take(4).collect();
        let mut seq = up.clone();
        if up.len() > 2 {
            seq.extend(up[1..up.len() - 1].iter().rev());
        }
        let count = ((end - p.start) / step).round() as usize;
        for i in 0..count {
            let t = p.start + i as f32 * step;
            out.push(Note::new(t, step * 0.95, seq[i % seq.len()] as u8, if i % 4 == 0 { 100 } else { 84 }));
        }
    }
    out
}

fn long_notes(project: &Project, cfg: &MelodyCfg) -> Vec<Note> {
    let mut rng = Rng::new(cfg.seed.wrapping_mul(2654435761).wrapping_add(4242));
    let pool = pool(cfg);
    let placed = project.placed();
    if pool.is_empty() || placed.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Note> = Vec::new();
    let mut prev = (cfg.lo + cfg.hi) / 2;
    for (ci, p) in placed.iter().enumerate() {
        let end = p.start + p.item_beats;
        let pcs = p.chord.pitch_classes();
        let tones: Vec<i32> = pool.iter().copied().filter(|n| pcs.contains(&(n.rem_euclid(12) as u8))).collect();
        if tones.is_empty() {
            continue;
        }
        let mut bar = 0.0;
        while p.start + bar < end - 0.001 {
            let t = p.start + bar;
            let len = (BEATS_PER_BAR).min(end - t);
            // Nearest chord tone, with a small random preference for moving rather than repeating.
            let mut choices: Vec<i32> = tones.clone();
            choices.sort_by_key(|n| (n - prev).abs());
            let pick = if rng.next_f32() < 0.4 && choices.len() > 1 { choices[1] } else { choices[0] };
            let last_of_sketch = ci + 1 == placed.len() && t + len >= end - 0.001;
            let pitch = if last_of_sketch {
                *tones.iter().filter(|n| n.rem_euclid(12) as u8 == p.chord.root).min_by_key(|n| (**n - prev).abs()).unwrap_or(&pick)
            } else {
                pick
            };
            // A passing note on the last beat when it is busy enough.
            if cfg.density > 0.55 && len >= 4.0 && !last_of_sketch {
                let i = pool.iter().position(|n| *n == pitch).unwrap_or(0);
                let neighbour = pool[(i + 1).min(pool.len() - 1)];
                out.push(Note::new(t, 3.0, pitch as u8, 96));
                out.push(Note::new(t + 3.0, 1.0, neighbour as u8, 84));
            } else {
                out.push(Note::new(t, len, pitch as u8, 96));
            }
            prev = pitch;
            bar += BEATS_PER_BAR;
        }
    }
    out
}

fn call_response(project: &Project, cfg: &MelodyCfg) -> Vec<Note> {
    let hook = generate_melody(project, cfg);
    let total = project.beats();
    let mut out: Vec<Note> = Vec::new();
    let bars = (total / BEATS_PER_BAR).ceil() as usize;
    for bar in 0..bars {
        let (a, b) = (bar as f32 * BEATS_PER_BAR, (bar as f32 + 1.0) * BEATS_PER_BAR);
        let in_bar: Vec<Note> = hook.iter().filter(|n| n.start >= a - 0.001 && n.start < b - 0.001).copied().collect();
        let is_answer = bar % 2 == 1;
        if !is_answer || in_bar.len() < 3 {
            out.extend(in_bar);
            continue;
        }
        // The answer: a short opening note and one long closing note.
        let first = in_bar[0];
        let last = *in_bar.last().unwrap();
        let close = (a + 2.0).max(last.start);
        let end_limit = project.chord_at(close).map_or(b, |p| (p.start + p.item_beats).min(b));
        out.push(Note::new(first.start, (close - first.start).min(first.len.max(0.5)).min(1.5), first.pitch, first.vel));
        out.push(Note::new(close, (end_limit - close).max(0.25), last.pitch, last.vel));
    }
    out
}

fn repitch(project: &Project, cfg: &MelodyCfg, existing: &[Note]) -> Vec<Note> {
    if existing.is_empty() {
        return generate_melody(project, cfg);
    }
    let mut rng = Rng::new(cfg.seed.wrapping_mul(2654435761).wrapping_add(9001));
    let pool = pool(cfg);
    if pool.is_empty() {
        return Vec::new();
    }
    let mut notes = existing.to_vec();
    notes.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut idx = pool.iter().position(|n| *n >= (cfg.lo + cfg.hi) / 2).unwrap_or(0);
    let mut out: Vec<Note> = Vec::new();
    let count = notes.len();
    for (i, n) in notes.iter().enumerate() {
        let chord = project.chord_at(n.start + 0.001).map(|p| p.chord);
        let strong = (n.start % 1.0).abs() < 0.001;
        let mut target = (idx as i32 + [-2, -1, -1, 0, 1, 1, 2][(rng.next_u32() % 7) as usize]).clamp(0, pool.len() as i32 - 1) as usize;
        let forbidden = match out.as_slice() {
            [.., a, b] if a.pitch == b.pitch => Some(b.pitch as i32),
            _ => None,
        };
        let last = i + 1 == count;
        if let Some(c) = chord.filter(|_| strong || last) {
            let pcs = c.pitch_classes();
            let want_root = last;
            let best = (-4i32..=4)
                .filter_map(|d| {
                    let j = target as i32 + d;
                    (j >= 0 && (j as usize) < pool.len()).then_some((d, j as usize))
                })
                .filter(|(_, j)| {
                    let pc = pool[*j].rem_euclid(12) as u8;
                    Some(pool[*j]) != forbidden && if want_root { pc == c.root } else { pcs.contains(&pc) }
                })
                .min_by_key(|(d, _)| d.abs());
            if let Some((_, j)) = best {
                target = j;
            }
        } else if Some(pool[target]) == forbidden {
            target = if target + 1 < pool.len() { target + 1 } else { target.saturating_sub(1) };
        }
        idx = target;
        out.push(Note::new(n.start, n.len, pool[idx] as u8, n.vel));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{TEMPLATES, GENRES};

    fn cfg(genre: Genre, seed: u32) -> MelodyCfg {
        MelodyCfg { key: 9, scale: Scale::Minor, genre, seed, density: 0.6, lo: 60, hi: 84 }
    }

    #[test]
    fn melody_is_in_range_in_key_and_monophonic() {
        for genre in GENRES {
            for seed in 0..20 {
                let p = starter();
                let m = generate_melody(&p, &cfg(genre, seed));
                assert!(!m.is_empty(), "{genre:?}");
                for n in &m {
                    assert!((60..=84).contains(&(n.pitch as i32)), "{genre:?} {n:?}");
                    assert!(in_scale(9, Scale::Minor, n.pitch % 12), "{genre:?} {n:?}");
                    assert!(n.end() <= 16.001);
                }
                for w in m.windows(2) {
                    assert!(w[1].start >= w[0].end() - 0.001, "overlap {genre:?}: {:?}", w);
                }
            }
        }
    }

    #[test]
    fn downbeats_are_chord_tones_and_the_end_resolves() {
        let p = starter();
        for seed in 0..30 {
            let m = generate_melody(&p, &cfg(Genre::Synthwave, seed));
            for n in &m {
                let chord = p.chord_at(n.start).unwrap().chord;
                if n.start.fract() == 0.0 && (n.start % 4.0 == 0.0) {
                    assert!(chord.contains_pc(n.pitch % 12), "seed {seed}: {n:?} over {}", chord.name(false));
                }
            }
            let last = m.last().unwrap();
            let last_chord = p.placed().last().unwrap().chord;
            assert_eq!(last.pitch % 12, last_chord.root, "seed {seed}");
        }
    }

    #[test]
    fn different_seeds_differ_and_same_seed_repeats() {
        let p = starter();
        let a = generate_melody(&p, &cfg(Genre::Synthwave, 1));
        let b = generate_melody(&p, &cfg(Genre::Synthwave, 1));
        let c = generate_melody(&p, &cfg(Genre::Synthwave, 2));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn every_style_writes_a_valid_melody_in_every_genre() {
        for style in MELODY_STYLES {
            for genre in GENRES {
                for seed in 0..8 {
                    let p = starter();
                    let c = cfg(genre, seed);
                    let existing = generate_melody(&p, &c);
                    let m = generate(&p, &c, style, &existing);
                    assert!(!m.is_empty(), "{style:?} {genre:?}");
                    for n in &m {
                        assert!((60..=84).contains(&(n.pitch as i32)), "{style:?} {n:?}");
                        assert!(in_scale(9, Scale::Minor, n.pitch % 12), "{style:?} {n:?}");
                        assert!(n.len > 0.0 && n.end() <= 16.001 && n.start >= 0.0, "{style:?} {n:?}");
                    }
                    for w in m.windows(2) {
                        assert!(w[1].start >= w[0].end() - 0.001, "{style:?} {genre:?} overlap {w:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn styles_have_their_character() {
        let p = starter();
        let c = cfg(Genre::Synthwave, 3);
        // riff: the same rhythm in every bar
        let riff = generate(&p, &c, MelodyStyle::Riff, &[]);
        let bar_rhythm = |bar: f32| riff.iter().filter(|n| n.start >= bar * 4.0 && n.start < bar * 4.0 + 4.0).map(|n| n.start - bar * 4.0).collect::<Vec<_>>();
        assert_eq!(bar_rhythm(0.0), bar_rhythm(1.0));
        assert_eq!(bar_rhythm(1.0), bar_rhythm(3.0));
        // arpeggio: only chord tones
        let arp = generate(&p, &c, MelodyStyle::Arp, &[]);
        for n in &arp {
            let chord = p.chord_at(n.start + 0.001).unwrap().chord;
            assert!(chord.contains_pc(n.pitch % 12), "{n:?} over {}", chord.name(false));
        }
        // long notes: few notes, all chord tones on bar starts
        let long = generate(&p, &c, MelodyStyle::Long, &[]);
        assert!(long.len() <= 8, "{}", long.len());
        // re-pitch keeps every start and length
        let mine = vec![Note::new(0.0, 1.0, 69, 100), Note::new(1.5, 0.5, 72, 90), Note::new(4.0, 2.0, 65, 100)];
        let re = generate(&p, &c, MelodyStyle::Repitch, &mine);
        assert_eq!(re.iter().map(|n| (n.start, n.len)).collect::<Vec<_>>(), mine.iter().map(|n| (n.start, n.len)).collect::<Vec<_>>());
        // call and answer: answer bars have at most two notes
        let ca = generate(&p, &c, MelodyStyle::CallResponse, &[]);
        let answer: Vec<_> = ca.iter().filter(|n| n.start >= 4.0 && n.start < 8.0).collect();
        assert!(answer.len() <= 2 || answer.len() < 4, "{answer:?}");
    }

    #[test]
    fn works_over_every_template() {
        for t in TEMPLATES {
            let mut p = Project::default();
            p.sections.push(t.section(0, "A"));
            let m = generate_melody(&p, &MelodyCfg { key: 0, scale: t.scale, genre: t.genre, seed: 3, density: 0.5, lo: 55, hi: 79 });
            assert!(!m.is_empty(), "{}", t.name);
        }
    }
}
