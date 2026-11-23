//! Turn the chord sketch into notes: the chord part, the bass part and (already stored) melody.
//! Playback and MIDI export both use this, so what you hear is exactly what you drag into the DAW.

use crate::project::*;
use crate::style::*;
use crate::theory::*;

#[derive(Clone, Copy, Debug)]
pub struct RenderCfg {
    pub key: u8,
    pub scale: Scale,
    pub chord_style: ChordStyle,
    pub bass_style: BassStyle,
    /// MIDI note the chords are centred around (60 = C4).
    pub center: i32,
    /// Open (drop-2) voicings.
    pub spread: bool,
    pub bass_range: (i32, i32),
    pub genre: Genre,
    /// Chosen drum pattern (index into `drums::PATTERNS`), or `None` for the genre's own.
    pub drum_pattern: Option<usize>,
    /// A fill on the last bar of each section.
    pub drum_fills: bool,
}

impl RenderCfg {
    pub fn new(key: u8, scale: Scale, genre: Genre) -> Self {
        let (chord_style, bass_style) = genre.default_styles();
        Self { key, scale, chord_style, bass_style, center: 60, spread: false, bass_range: genre.bass_range(), genre, drum_pattern: None, drum_fills: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Rendered {
    pub chords: Vec<Note>,
    pub bass: Vec<Note>,
    pub melody: Vec<Note>,
    pub drums: Vec<Note>,
}

impl Rendered {
    pub fn all(&self) -> impl Iterator<Item = &Note> {
        self.chords.iter().chain(self.bass.iter()).chain(self.melody.iter()).chain(self.drums.iter())
    }
}

/// The notes of every chord (in project order) with voice leading applied.
pub fn voicings(project: &Project, cfg: &RenderCfg) -> Vec<(Placed, Vec<u8>)> {
    let mut prev: Vec<u8> = Vec::new();
    project
        .placed()
        .into_iter()
        .map(|p| {
            let notes = match p.inversion {
                Some(inv) => chord_notes(&p.chord, inv, cfg.spread, cfg.center),
                None => voice_lead(&prev, &p.chord, cfg.spread, cfg.center).0,
            };
            prev = notes.clone();
            (p, notes)
        })
        .collect()
}

pub fn render(project: &Project, cfg: &RenderCfg) -> Rendered {
    let mut out = Rendered { melody: project.melody.clone(), ..Default::default() };
    out.drums = crate::drums::render_drums(project, cfg.genre, crate::drums::DrumMode::Notes, cfg.drum_pattern, cfg.drum_fills);
    for (placed, notes) in voicings(project, cfg) {
        let (start, end) = (placed.start, placed.start + placed.item_beats);
        render_chord(&mut out.chords, &notes, start, end, cfg.chord_style);
        render_bass(&mut out.bass, &placed.chord, start, end, cfg);
    }
    out
}

fn render_chord(out: &mut Vec<Note>, notes: &[u8], start: f32, end: f32, style: ChordStyle) {
    if notes.is_empty() {
        return;
    }
    if style.is_arp() {
        let mut pool: Vec<u8> = notes.to_vec();
        if pool.len() < 4 {
            pool.push(notes[0].saturating_add(12).min(127));
        }
        let n = pool.len();
        let cycle: Vec<usize> = match style {
            ChordStyle::ArpUpDown if n > 2 => (0..n).chain((1..n - 1).rev()).collect(),
            _ => (0..n).collect(),
        };
        let steps = ((end - start) * 4.0).round() as usize;
        for i in 0..steps {
            let t = start + i as f32 * 0.25;
            out.push(Note::new(t, 0.25, pool[cycle[i % cycle.len()]], if i % 4 == 0 { 100 } else { 84 }));
        }
        return;
    }
    if style == ChordStyle::Sustain {
        for &p in notes {
            out.push(Note::new(start, end - start, p, 92));
        }
        return;
    }
    let pattern = style.bar_pattern();
    let mut bar = 0.0;
    while start + bar < end - 0.001 {
        for (i, &(s, l)) in pattern.iter().enumerate() {
            let t = start + bar + s;
            if t >= end - 0.001 {
                break;
            }
            let len = l.min(end - t);
            for &p in notes {
                out.push(Note::new(t, len, p, if i == 0 { 100 } else { 82 }));
            }
        }
        bar += BEATS_PER_BAR;
    }
}

fn render_bass(out: &mut Vec<Note>, chord: &Chord, start: f32, end: f32, cfg: &RenderCfg) {
    let pattern = cfg.bass_style.bar_pattern();
    if pattern.is_empty() {
        return;
    }
    let pc = chord.bass.unwrap_or(chord.root);
    let (lo, hi) = cfg.bass_range;
    let base = pc_in_range(pc, lo, hi);
    if cfg.bass_style == BassStyle::Sustain {
        out.push(Note::new(start, end - start, base.clamp(0, 127) as u8, 100));
        return;
    }
    let mut bar = 0.0;
    while start + bar < end - 0.001 {
        for (i, &(s, l, up)) in pattern.iter().enumerate() {
            let t = start + bar + s;
            if t >= end - 0.001 {
                break;
            }
            let pitch = (base + up).clamp(0, 127) as u8;
            out.push(Note::new(t, l.min(end - t), pitch, if i == 0 { 108 } else if up > 0 { 84 } else { 96 }));
        }
        bar += BEATS_PER_BAR;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(style: ChordStyle, bass: BassStyle) -> RenderCfg {
        RenderCfg { chord_style: style, bass_style: bass, ..RenderCfg::new(9, Scale::Minor, Genre::Any) }
    }

    #[test]
    fn sustained_chords_fill_the_whole_sketch() {
        let p = starter();
        let r = render(&p, &cfg(ChordStyle::Sustain, BassStyle::Sustain));
        assert_eq!(r.chords.len(), 12); // 4 triads
        assert_eq!(r.bass.len(), 4);
        let end = r.all().map(Note::end).fold(0.0, f32::max);
        assert_eq!(end, 16.0);
        // the bass of the first chord (Am) is an A
        assert_eq!(r.bass[0].pitch % 12, 9);
        assert!(r.bass.iter().all(|n| (33..=45).contains(&(n.pitch as i32))));
    }

    #[test]
    fn patterns_stay_inside_their_chord() {
        let p = starter();
        for style in CHORD_STYLES {
            for bass in BASS_STYLES {
                let r = render(&p, &cfg(style, bass));
                for n in r.chords.iter().chain(&r.bass) {
                    assert!(n.start >= 0.0 && n.end() <= 16.001, "{style:?}/{bass:?}: {n:?}");
                    assert!(n.len > 0.0);
                }
            }
        }
    }

    #[test]
    fn short_chords_are_cut_at_the_boundary() {
        let mut p = Project::default();
        p.push_chord(Chord::new(0, Quality::Maj), 1.5);
        let r = render(&p, &cfg(ChordStyle::Pulse, BassStyle::Eighths));
        assert!(r.chords.iter().all(|n| n.end() <= 1.501));
        assert_eq!(r.bass.len(), 3);
    }

    #[test]
    fn arp_cycles_through_the_chord() {
        let mut p = Project::default();
        p.push_chord(Chord::new(0, Quality::Maj), 4.0);
        let r = render(&p, &cfg(ChordStyle::ArpUp, BassStyle::Off));
        assert_eq!(r.chords.len(), 16);
        assert!(r.bass.is_empty());
        let pcs: Vec<u8> = r.chords.iter().take(4).map(|n| n.pitch % 12).collect();
        assert_eq!(pcs, vec![0, 4, 7, 0]);
    }
}
