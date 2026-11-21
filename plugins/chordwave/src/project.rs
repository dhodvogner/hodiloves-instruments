//! The song sketch: sections of chords plus a melody, shared between editor, host state and audio thread.

use crate::theory::*;
use nih_plug::params::persist::PersistentField;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

/// Everything is 4/4: synthwave and drum and bass both live there.
pub const BEATS_PER_BAR: f32 = 4.0;
pub const MIN_BEATS: f32 = 0.5;
pub const MAX_BEATS: f32 = 32.0;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Note {
    /// Start in beats (quarter notes) from the start of the sketch.
    pub start: f32,
    pub len: f32,
    pub pitch: u8,
    pub vel: u8,
}

impl Note {
    pub fn new(start: f32, len: f32, pitch: u8, vel: u8) -> Self {
        Self { start, len, pitch, vel }
    }
    pub fn end(&self) -> f32 {
        self.start + self.len
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ChordItem {
    pub chord: Chord,
    pub beats: f32,
    /// `None`: choose the inversion automatically for smooth voice leading.
    pub inversion: Option<u8>,
}

impl ChordItem {
    pub fn new(chord: Chord, beats: f32) -> Self {
        Self { chord, beats, inversion: None }
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub items: Vec<ChordItem>,
}

impl Section {
    pub fn beats(&self) -> f32 {
        self.items.iter().map(|i| i.beats).sum()
    }
}

/// Position of a chord in the sketch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sel {
    pub section: usize,
    pub item: usize,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Project {
    pub sections: Vec<Section>,
    pub melody: Vec<Note>,
}

/// One chord placed on the timeline.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub sel: Sel,
    pub start: f32,
    pub item_beats: f32,
    pub chord: Chord,
    pub inversion: Option<u8>,
}

impl Project {
    pub fn beats(&self) -> f32 {
        self.sections.iter().map(Section::beats).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.iter().all(|s| s.items.is_empty())
    }

    pub fn placed(&self) -> Vec<Placed> {
        let mut out = Vec::new();
        let mut t = 0.0;
        for (si, s) in self.sections.iter().enumerate() {
            for (ii, it) in s.items.iter().enumerate() {
                out.push(Placed {
                    sel: Sel { section: si, item: ii },
                    start: t,
                    item_beats: it.beats,
                    chord: it.chord,
                    inversion: it.inversion,
                });
                t += it.beats;
            }
        }
        out
    }

    pub fn start_of(&self, sel: Sel) -> f32 {
        self.placed().iter().find(|p| p.sel == sel).map(|p| p.start).unwrap_or(0.0)
    }

    pub fn item(&self, sel: Sel) -> Option<&ChordItem> {
        self.sections.get(sel.section)?.items.get(sel.item)
    }

    pub fn item_mut(&mut self, sel: Sel) -> Option<&mut ChordItem> {
        self.sections.get_mut(sel.section)?.items.get_mut(sel.item)
    }

    pub fn chord_at(&self, beat: f32) -> Option<Placed> {
        self.placed().into_iter().find(|p| beat >= p.start && beat < p.start + p.item_beats)
    }

    /// Make room for (`delta` > 0) or close up (`delta` < 0) `delta` beats at `at` in the melody, so
    /// the melody stays attached to its chords when chords are inserted, removed or resized.
    pub fn ripple_melody(&mut self, at: f32, delta: f32) {
        const EPS: f32 = 1e-4;
        if delta >= 0.0 {
            for n in &mut self.melody {
                if n.start >= at - EPS {
                    n.start += delta;
                }
            }
        } else {
            let cut = at + delta;
            self.melody.retain(|n| !(n.start >= cut - EPS && n.start < at - EPS));
            for n in &mut self.melody {
                if n.start >= at - EPS {
                    n.start += delta;
                } else if n.end() > cut {
                    n.len = (cut - n.start).max(0.0625);
                }
            }
        }
    }

    /// Copy the melody that starts inside `start..start+len` to `start+len..` (after the gap has been made).
    fn copy_melody(&mut self, start: f32, len: f32) {
        let copies: Vec<Note> = self.melody_in(start, start + len).into_iter().map(|n| Note { start: n.start + len, ..n }).collect();
        self.melody.extend(copies);
    }

    /// Append a chord to the last section (creating one if the sketch is empty).
    pub fn push_chord(&mut self, chord: Chord, beats: f32) -> Sel {
        if self.sections.is_empty() {
            self.sections.push(Section { name: "A".into(), items: Vec::new() });
        }
        let si = self.sections.len() - 1;
        let at = self.sections[si].items.len();
        self.insert_item(si, at, ChordItem::new(chord, beats))
    }

    /// Insert a chord item at `index` in a section. Melody after that point moves later.
    pub fn insert_item(&mut self, section: usize, index: usize, item: ChordItem) -> Sel {
        if self.sections.is_empty() {
            self.sections.push(Section { name: "A".into(), items: Vec::new() });
        }
        let section = section.min(self.sections.len() - 1);
        let index = index.min(self.sections[section].items.len());
        let at = self.start_of_slot(section, index);
        self.ripple_melody(at, item.beats);
        self.sections[section].items.insert(index, item);
        Sel { section, item: index }
    }

    fn start_of_slot(&self, section: usize, index: usize) -> f32 {
        let before: f32 = self.sections[..section].iter().map(Section::beats).sum();
        before + self.sections[section].items[..index].iter().map(|i| i.beats).sum::<f32>()
    }

    /// Insert a chord so that it starts at (or just before) `beat`, inside the section under it.
    pub fn insert_chord_at(&mut self, beat: f32, chord: Chord, beats: f32) -> Sel {
        let mut t = 0.0;
        for si in 0..self.sections.len() {
            let mut at = 0;
            for it in self.sections[si].items.iter() {
                if beat < t + it.beats * 0.5 {
                    break;
                }
                t += it.beats;
                at += 1;
            }
            if at < self.sections[si].items.len() || beat < t + 0.01 {
                return self.insert_item(si, at, ChordItem::new(chord, beats));
            }
        }
        self.push_chord(chord, beats)
    }

    pub fn remove(&mut self, sel: Sel) {
        let Some(item) = self.item(sel).cloned() else { return };
        let start = self.start_of(sel);
        self.sections[sel.section].items.remove(sel.item);
        self.ripple_melody(start + item.beats, -item.beats);
        if self.sections[sel.section].items.is_empty() && self.sections.len() > 1 {
            self.sections.remove(sel.section);
        }
    }

    /// Delete a whole section with its chords and melody (the last section is only emptied).
    pub fn remove_section(&mut self, index: usize) {
        let Some(s) = self.sections.get(index) else { return };
        let beats = s.beats();
        let start = self.start_of_slot(index, 0);
        if self.sections.len() > 1 {
            self.sections.remove(index);
        } else {
            self.sections[index].items.clear();
        }
        self.ripple_melody(start + beats, -beats);
    }

    /// Swap a chord with its neighbour (`dir` = -1 left, +1 right) inside its section. The melody stays where it is.
    pub fn move_item(&mut self, sel: Sel, dir: i32) -> Sel {
        let Some(s) = self.sections.get_mut(sel.section) else { return sel };
        let target = sel.item as i32 + dir;
        if target >= 0 && (target as usize) < s.items.len() {
            s.items.swap(sel.item, target as usize);
            return Sel { section: sel.section, item: target as usize };
        }
        sel
    }

    /// Duplicate a chord right after itself, together with the melody over it.
    pub fn duplicate_item(&mut self, sel: Sel) -> Sel {
        let Some(it) = self.item(sel).cloned() else { return sel };
        let start = self.start_of(sel);
        let new = self.insert_item(sel.section, sel.item + 1, it.clone());
        self.copy_melody(start, it.beats);
        new
    }

    /// Duplicate a whole section (chords and melody) right after itself: ready for the next verse.
    pub fn duplicate_section(&mut self, index: usize) {
        let Some(s) = self.sections.get(index).cloned() else { return };
        let (start, beats) = (self.start_of_slot(index, 0), s.beats());
        self.ripple_melody(start + beats, beats);
        let mut copy = s;
        copy.name = format!("{}'", copy.name);
        self.sections.insert(index + 1, copy);
        self.copy_melody(start, beats);
    }

    pub fn add_section(&mut self) -> usize {
        let name = ((b'A' + (self.sections.len() % 26) as u8) as char).to_string();
        self.sections.push(Section { name, items: Vec::new() });
        self.sections.len() - 1
    }

    /// Change the length of one chord. Melody after it moves along; melody inside a removed part is dropped.
    pub fn set_beats(&mut self, sel: Sel, beats: f32) {
        let start = self.start_of(sel);
        let Some(it) = self.item_mut(sel) else { return };
        let old = it.beats;
        let new = beats.clamp(MIN_BEATS, MAX_BEATS);
        it.beats = new;
        if (new - old).abs() > 1e-4 {
            self.ripple_melody(start + old, new - old);
        }
    }

    /// Notes of `melody` that start inside the given beat range.
    pub fn melody_in(&self, a: f32, b: f32) -> Vec<Note> {
        self.melody.iter().filter(|n| n.start >= a - 0.001 && n.start < b - 0.001).copied().collect()
    }

    /// Drop melody notes that start after the end of the sketch and trim those that run past it.
    pub fn clamp_melody(&mut self) {
        let end = self.beats().max(BEATS_PER_BAR);
        self.melody.retain(|n| n.start < end);
        for n in &mut self.melody {
            n.len = n.len.min(end - n.start).max(0.0625);
        }
    }
}

/// A starter progression so the plugin is useful the moment it opens (A minor: Am F C G).
pub fn starter() -> Project {
    let c = |d, e| diatonic_chord(9, Scale::Minor, d, e);
    Project {
        sections: vec![Section {
            name: "A".into(),
            items: [(0, Ext::Triad), (5, Ext::Triad), (2, Ext::Triad), (6, Ext::Triad)]
                .iter()
                .map(|&(d, e)| ChordItem::new(c(d, e), 4.0))
                .collect(),
        }],
        melody: Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------
// Shared cell: the editor edits it, the host saves and restores it
// ---------------------------------------------------------------------------------------------

pub struct ProjectCell {
    inner: RwLock<Project>,
    version: AtomicU32,
}

impl Default for ProjectCell {
    fn default() -> Self {
        Self { inner: RwLock::new(starter()), version: AtomicU32::new(1) }
    }
}

impl ProjectCell {
    pub fn version(&self) -> u32 {
        self.version.load(Ordering::Acquire)
    }

    pub fn get(&self) -> Project {
        self.inner.read().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn set(&self, project: Project) {
        if let Ok(mut p) = self.inner.write() {
            *p = project;
        }
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    /// Edit in place and bump the version when the closure reports a change.
    pub fn edit<R>(&self, f: impl FnOnce(&mut Project) -> R) -> R {
        let mut guard = self.inner.write().unwrap_or_else(|e| e.into_inner());
        let before = guard.clone();
        let r = f(&mut guard);
        if *guard != before {
            self.version.fetch_add(1, Ordering::AcqRel);
        }
        r
    }
}

impl<'a> PersistentField<'a, Project> for ProjectCell {
    fn set(&self, new_value: Project) {
        ProjectCell::set(self, new_value);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Project) -> R,
    {
        f(&self.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_and_editing() {
        let mut p = starter();
        assert_eq!(p.beats(), 16.0);
        assert_eq!(p.placed()[2].start, 8.0);
        let sel = p.insert_chord_at(6.0, Chord::new(0, Quality::Maj), 2.0);
        assert_eq!(sel, Sel { section: 0, item: 2 });
        assert_eq!(p.beats(), 18.0);
        p.remove(sel);
        assert_eq!(p.beats(), 16.0);
        let s = Sel { section: 0, item: 0 };
        let moved = p.move_item(s, 1);
        assert_eq!(moved.item, 1);
        assert_eq!(p.chord_at(4.5).unwrap().sel, moved);
    }

    #[test]
    fn melody_follows_chord_edits() {
        let mut p = starter(); // Am F C G, 4 beats each
        p.melody = vec![Note::new(0.0, 1.0, 69, 100), Note::new(4.0, 1.0, 65, 100), Note::new(8.0, 1.0, 64, 100), Note::new(12.0, 1.0, 62, 100)];

        // insert a 2-beat chord before chord 2: melody from beat 4 on moves 2 beats later
        let mut q = p.clone();
        q.insert_item(0, 1, ChordItem::new(Chord::new(0, Quality::Maj), 2.0));
        assert_eq!(q.melody.iter().map(|n| n.start).collect::<Vec<_>>(), vec![0.0, 6.0, 10.0, 14.0]);

        // removing chord 2 removes its melody note and pulls the rest back
        let mut q = p.clone();
        q.remove(Sel { section: 0, item: 1 });
        assert_eq!(q.melody.iter().map(|n| n.start).collect::<Vec<_>>(), vec![0.0, 4.0, 8.0]);
        assert_eq!(q.melody.iter().map(|n| n.pitch).collect::<Vec<_>>(), vec![69, 64, 62]);

        // making chord 1 longer pushes the rest; shortening it drops what falls off the end
        let mut q = p.clone();
        q.set_beats(Sel { section: 0, item: 0 }, 6.0);
        assert_eq!(q.melody.iter().map(|n| n.start).collect::<Vec<_>>(), vec![0.0, 6.0, 10.0, 14.0]);
        let mut q = p.clone();
        q.set_beats(Sel { section: 0, item: 1 }, 2.0);
        assert_eq!(q.melody.iter().map(|n| n.start).collect::<Vec<_>>(), vec![0.0, 4.0, 6.0, 10.0]);

        // duplicating a section copies its melody too
        let mut q = p.clone();
        q.duplicate_section(0);
        assert_eq!(q.beats(), 32.0);
        assert_eq!(q.melody.len(), 8);
        assert_eq!(q.melody[4].start, 16.0);
        assert_eq!(q.melody[4].pitch, 69);

        // duplicating one chord copies the notes over it
        let mut q = p.clone();
        q.duplicate_item(Sel { section: 0, item: 0 });
        assert_eq!(q.melody.iter().filter(|n| n.pitch == 69).count(), 2);
        assert_eq!(q.melody.iter().filter(|n| n.pitch == 65).count(), 1);
        assert_eq!(q.melody.iter().find(|n| n.pitch == 65).unwrap().start, 8.0);
    }

    #[test]
    fn project_round_trips_through_json() {
        let mut p = starter();
        p.melody.push(Note::new(0.0, 1.0, 69, 100));
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn cell_versions_only_on_change() {
        let cell = ProjectCell::default();
        let v = cell.version();
        cell.edit(|_| {});
        assert_eq!(cell.version(), v);
        cell.edit(|p| p.sections[0].name = "Verse".into());
        assert_eq!(cell.version(), v + 1);
    }
}
