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
    fn works_over_every_template() {
        for t in TEMPLATES {
            let mut p = Project::default();
            p.sections.push(t.section(0, "A"));
            let m = generate_melody(&p, &MelodyCfg { key: 0, scale: t.scale, genre: t.genre, seed: 3, density: 0.5, lo: 55, hi: 79 });
            assert!(!m.is_empty(), "{}", t.name);
        }
    }
}
