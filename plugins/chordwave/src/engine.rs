//! Playback: turns the rendered parts into sample-accurate note on / off events that follow the
//! host transport (or the built-in clock). Everything here is allocation-free once running.

use crate::project::*;
use crate::render::*;

pub const MAX_NOTES: usize = 16_384;
const MAX_ACTIVE: usize = 1024;
const MAX_EVENTS: usize = 2048;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct RNote {
    pub start: f32,
    pub end: f32,
    pub pitch: u8,
    pub vel: u8,
    pub part: u8,
}

/// The sketch rendered for playback: notes sorted by start time and the loop length in beats.
#[derive(Clone, Default)]
pub struct Playback {
    pub notes: Vec<RNote>,
    pub len: f32,
}

pub fn build_playback(project: &Project, cfg: &RenderCfg) -> Playback {
    let r = render(project, cfg);
    let mut notes: Vec<RNote> = Vec::with_capacity(r.chords.len() + r.bass.len() + r.melody.len());
    for (part, list) in [(0u8, &r.chords), (1, &r.bass), (2, &r.melody)] {
        notes.extend(list.iter().map(|n| RNote { start: n.start, end: n.end(), pitch: n.pitch, vel: n.vel, part }));
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    notes.truncate(MAX_NOTES);
    let bars = (project.beats() / BEATS_PER_BAR).ceil().max(1.0);
    Playback { notes, len: bars * BEATS_PER_BAR }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ev {
    pub offset: u32,
    pub on: bool,
    pub pitch: u8,
    pub vel: u8,
    pub part: u8,
}

#[derive(Clone, Copy)]
struct Active {
    pitch: u8,
    part: u8,
    end: f64,
    /// Sample offset of the note-on inside the current block, if it started in it.
    on_offset: Option<u32>,
}

pub struct Engine {
    notes: Vec<RNote>,
    len: f32,
    active: Vec<Active>,
    pub events: Vec<Ev>,
    expected: Option<f64>,
    /// Beat of the built-in clock (used when the host is not playing).
    pub internal_beat: f64,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            notes: Vec::with_capacity(MAX_NOTES),
            len: 0.0,
            active: Vec::with_capacity(MAX_ACTIVE),
            events: Vec::with_capacity(MAX_EVENTS),
            expected: None,
            internal_beat: 0.0,
        }
    }
}

impl Engine {
    /// Replace the notes. Capacity is reserved up front, so this never allocates.
    pub fn set_playback(&mut self, p: &Playback) {
        self.notes.clear();
        let n = p.notes.len().min(MAX_NOTES);
        self.notes.extend_from_slice(&p.notes[..n]);
        self.len = p.len;
    }

    pub fn len(&self) -> f32 {
        self.len
    }

    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    pub fn push(&mut self, ev: Ev) {
        if self.events.len() < MAX_EVENTS {
            self.events.push(ev);
        }
    }

    /// Send a note-off for everything that is sounding.
    pub fn release_all(&mut self, offset: u32) {
        for i in 0..self.active.len() {
            let a = self.active[i];
            self.push(Ev { offset, on: false, pitch: a.pitch, vel: 0, part: a.part });
        }
        self.active.clear();
    }

    pub fn reset(&mut self) {
        self.active.clear();
        self.events.clear();
        self.expected = None;
    }

    /// The position inside the loop (beats) for a song position, or `None` past the end of a one-shot.
    pub fn loop_pos(&self, beat: f64, repeat: bool) -> Option<f32> {
        if self.len <= 0.0 {
            return None;
        }
        if repeat {
            Some(beat.rem_euclid(self.len as f64) as f32)
        } else if beat < self.len as f64 {
            Some(beat as f32)
        } else {
            None
        }
    }

    /// Work out every note-on / note-off in a block of `n` samples starting at song position
    /// `beat0`. The result is in `self.events`, sorted by time with note-offs first.
    pub fn process_block(&mut self, beat0: f64, n: usize, beats_per_sample: f64, playing: bool, repeat: bool, parts_on: [bool; 3]) {
        self.events.clear();
        if !playing || self.len <= 0.0 || n == 0 {
            if !self.active.is_empty() {
                self.release_all(0);
            }
            self.expected = None;
            return;
        }
        let mut beat0 = beat0;
        match self.expected {
            Some(e) if (e - beat0).abs() < 0.01 => beat0 = e, // contiguous: use our own running value
            Some(_) => self.release_all(0),                   // the host jumped (loop, locate, ...)
            None => {}
        }
        let end = beat0 + n as f64 * beats_per_sample;
        let len = self.len as f64;
        let to_offset = |abs: f64| (((abs - beat0) / beats_per_sample).round().max(0.0) as usize).min(n - 1) as u32;

        let mut seg = beat0;
        while seg < end - 1e-9 {
            let lb = if repeat { seg.rem_euclid(len) } else { seg };
            if lb >= len {
                break;
            }
            let seg_end = (seg + (len - lb)).min(end);
            let dur = seg_end - seg;
            let first = self.notes.partition_point(|x| (x.start as f64) < lb - 1e-6);
            let mut i = first;
            while i < self.notes.len() && (self.notes[i].start as f64) < lb + dur - 1e-6 {
                let note = self.notes[i];
                i += 1;
                if !parts_on[(note.part as usize).min(2)] {
                    continue;
                }
                let abs_start = (seg + (note.start as f64 - lb)).max(seg);
                let offset = to_offset(abs_start);
                self.push(Ev { offset, on: true, pitch: note.pitch, vel: note.vel, part: note.part });
                if self.active.len() < MAX_ACTIVE {
                    self.active.push(Active {
                        pitch: note.pitch,
                        part: note.part,
                        end: abs_start + (note.end - note.start) as f64,
                        on_offset: Some(offset),
                    });
                }
            }
            seg = seg_end;
        }

        // Notes that end inside this block (a note-off never comes before its own note-on).
        let mut i = 0;
        while i < self.active.len() {
            let a = self.active[i];
            if a.end < end {
                let mut offset = to_offset(a.end);
                if let Some(on) = a.on_offset {
                    offset = offset.max(on + 1);
                }
                if offset as usize <= n - 1 {
                    self.push(Ev { offset, on: false, pitch: a.pitch, vel: 0, part: a.part });
                    self.active.swap_remove(i);
                    continue;
                }
            }
            self.active[i].on_offset = None;
            i += 1;
        }
        self.events.sort_unstable_by_key(|e| (e.offset, e.on as u8, e.pitch));
        self.expected = Some(end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::*;
    use crate::theory::*;

    fn engine_for(project: &Project) -> Engine {
        let cfg = RenderCfg { key: 9, scale: Scale::Minor, chord_style: ChordStyle::Sustain, bass_style: BassStyle::Sustain, center: 60, spread: false, bass_range: (33, 45) };
        let mut e = Engine::default();
        e.set_playback(&build_playback(project, &cfg));
        e
    }

    /// Run `blocks` blocks of 480 samples at 120 bpm (1 beat = 24000 samples at 48 kHz) and collect (absolute sample, event).
    fn run(e: &mut Engine, start_beat: f64, blocks: usize, repeat: bool) -> Vec<(u64, Ev)> {
        let bps = 120.0 / 60.0 / 48_000.0;
        let mut out = Vec::new();
        for b in 0..blocks {
            let beat0 = start_beat + (b * 480) as f64 * bps;
            e.process_block(beat0, 480, bps, true, repeat, [true; 3]);
            out.extend(e.events.iter().map(|ev| ((b * 480) as u64 + ev.offset as u64, *ev)));
        }
        out
    }

    #[test]
    fn every_note_on_gets_exactly_one_off() {
        let p = starter();
        let mut e = engine_for(&p);
        // 16 beats = 384000 samples = 800 blocks; run 3 loops
        let evs = run(&mut e, 0.0, 2500, true);
        let ons = evs.iter().filter(|(_, e)| e.on).count();
        let offs = evs.iter().filter(|(_, e)| !e.on).count();
        assert!(ons > 30, "{ons}");
        assert!(offs >= ons - 16 && offs <= ons, "ons {ons} offs {offs}"); // the notes still ringing at the end are not off yet
    }

    #[test]
    fn first_chord_starts_at_sample_zero_and_ends_at_four_beats() {
        let p = starter();
        let mut e = engine_for(&p);
        let evs = run(&mut e, 0.0, 400, true);
        let first_on: Vec<_> = evs.iter().filter(|(t, ev)| *t == 0 && ev.on).collect();
        assert_eq!(first_on.len(), 4, "3 chord notes + bass"); // Am triad + bass
        // chord 1 ends at beat 4 = 96000 samples; chord 2 starts then (off before on)
        let at_4: Vec<_> = evs.iter().filter(|(t, _)| *t == 96_000 || *t == 96_000 - 1 || *t == 96_000 + 1).collect();
        assert!(at_4.iter().any(|(_, ev)| !ev.on) && at_4.iter().any(|(_, ev)| ev.on));
    }

    #[test]
    fn loop_wraps_and_repeats_the_same_notes() {
        let p = starter();
        let mut e = engine_for(&p);
        let evs = run(&mut e, 0.0, 1700, true); // > 2 loops of 800 blocks
        let starts_at_zero = evs.iter().filter(|(t, ev)| ev.on && (*t % 384_000) == 0).count();
        assert_eq!(starts_at_zero, 12, "first beat of loop 1, 2 and 3: 4 notes each");
    }

    #[test]
    fn stopping_releases_everything() {
        let p = starter();
        let mut e = engine_for(&p);
        run(&mut e, 0.0, 10, true);
        assert!(e.has_active());
        e.process_block(0.0, 480, 1e-4, false, true, [true; 3]);
        assert!(e.events.iter().all(|ev| !ev.on) && !e.events.is_empty());
        assert!(!e.has_active());
    }

    #[test]
    fn a_jump_releases_and_restarts() {
        let p = starter();
        let mut e = engine_for(&p);
        run(&mut e, 0.0, 10, true);
        let bps = 2.0 / 48_000.0;
        e.process_block(8.0, 480, bps, true, true, [true; 3]);
        assert!(e.events.iter().any(|ev| !ev.on && ev.offset == 0));
        assert!(e.events.iter().any(|ev| ev.on && ev.offset == 0)); // chord 3 begins at beat 8
    }

    #[test]
    fn one_shot_stops_at_the_end_and_muted_parts_stay_silent() {
        let p = starter();
        let mut e = engine_for(&p);
        let evs = run(&mut e, 0.0, 1700, false);
        let ons = evs.iter().filter(|(_, ev)| ev.on).count();
        assert_eq!(ons, 16); // 4 chords x (3 notes + bass), once
        let mut e = engine_for(&p);
        e.process_block(0.0, 480, 1e-4, true, true, [false, true, false]);
        assert!(e.events.iter().all(|ev| ev.part == 1));
    }

    #[test]
    fn short_notes_never_turn_off_before_they_turn_on() {
        let mut p = Project::default();
        p.push_chord(Chord::new(0, Quality::Maj), 4.0);
        let cfg = RenderCfg { key: 0, scale: Scale::Major, chord_style: ChordStyle::ArpUp, bass_style: BassStyle::Rolling, center: 60, spread: false, bass_range: (28, 40) };
        let mut e = Engine::default();
        e.set_playback(&build_playback(&p, &cfg));
        // huge blocks relative to the note length
        let bps = 2.0 / 48_000.0;
        let mut count: std::collections::HashMap<(u8, u8), i32> = Default::default();
        let mut beat = 0.0;
        for _ in 0..200 {
            e.process_block(beat, 4096, bps, true, true, [true; 3]);
            for ev in &e.events {
                let c = count.entry((ev.part, ev.pitch)).or_insert(0);
                *c += if ev.on { 1 } else { -1 };
                assert!(*c >= 0, "note-off before its note-on: {ev:?}");
            }
            beat += 4096.0 * bps;
        }
        // after releasing everything nothing must hang
        e.process_block(0.0, 64, bps, false, true, [true; 3]);
        for ev in &e.events {
            *count.entry((ev.part, ev.pitch)).or_insert(0) += if ev.on { 1 } else { -1 };
        }
        assert!(!e.has_active());
        assert!(count.values().all(|c| *c == 0), "hanging notes: {count:?}");
    }
}
