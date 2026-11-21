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

    /// Append a chord to the last section (creating one if the sketch is empty).
    pub fn push_chord(&mut self, chord: Chord, beats: f32) -> Sel {
        if self.sections.is_empty() {
            self.sections.push(Section { name: "A".into(), items: Vec::new() });
        }
        let si = self.sections.len() - 1;
        self.sections[si].items.push(ChordItem::new(chord, beats));
        Sel { section: si, item: self.sections[si].items.len() - 1 }
    }

    /// Insert a chord so that it starts at (or just before) `beat`, inside the section under it.
    pub fn insert_chord_at(&mut self, beat: f32, chord: Chord, beats: f32) -> Sel {
        let mut t = 0.0;
        for (si, s) in self.sections.iter_mut().enumerate() {
            let mut at = 0;
            for it in s.items.iter() {
                if beat < t + it.beats * 0.5 {
                    break;
                }
                t += it.beats;
                at += 1;
            }
            if at < s.items.len() || beat < t + 0.01 {
                s.items.insert(at, ChordItem::new(chord, beats));
                return Sel { section: si, item: at };
            }
        }
        self.push_chord(chord, beats)
    }

    pub fn remove(&mut self, sel: Sel) {
        if let Some(s) = self.sections.get_mut(sel.section) {
            if sel.item < s.items.len() {
                s.items.remove(sel.item);
            }
            if s.items.is_empty() && self.sections.len() > 1 {
                self.sections.remove(sel.section);
            }
        }
    }

    /// Swap a chord with its neighbour (`dir` = -1 left, +1 right), crossing section borders.
    pub fn move_item(&mut self, sel: Sel, dir: i32) -> Sel {
        let Some(s) = self.sections.get_mut(sel.section) else { return sel };
        let target = sel.item as i32 + dir;
        if target >= 0 && (target as usize) < s.items.len() {
            s.items.swap(sel.item, target as usize);
            return Sel { section: sel.section, item: target as usize };
        }
        sel
    }

    /// Duplicate a chord right after itself.
    pub fn duplicate_item(&mut self, sel: Sel) -> Sel {
        if let Some(it) = self.item(sel).cloned() {
            self.sections[sel.section].items.insert(sel.item + 1, it);
            return Sel { section: sel.section, item: sel.item + 1 };
        }
        sel
    }

    /// Duplicate a whole section (chords only) right after itself.
    pub fn duplicate_section(&mut self, index: usize) {
        if let Some(s) = self.sections.get(index).cloned() {
            let mut copy = s;
            copy.name = format!("{}'", copy.name);
            self.sections.insert(index + 1, copy);
        }
    }

    pub fn add_section(&mut self) -> usize {
        let name = ((b'A' + (self.sections.len() % 26) as u8) as char).to_string();
        self.sections.push(Section { name, items: Vec::new() });
        self.sections.len() - 1
    }

    /// Change the length of one chord, snapped to a beat grid.
    pub fn set_beats(&mut self, sel: Sel, beats: f32) {
        if let Some(it) = self.item_mut(sel) {
            it.beats = beats.clamp(MIN_BEATS, MAX_BEATS);
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
