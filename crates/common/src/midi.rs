//! Lock-free MIDI plumbing shared by the editors, the standalone launchers and the audio thread.
//!
//! Two things feed notes into a plugin besides the host: the on-screen keyboard in the editor and,
//! in the standalone apps, every connected MIDI input port. Both push raw three-byte messages into
//! a [`MidiQueue`]; the audio thread drains it at the start of each process call.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;

const CAP: usize = 256;

/// Multi-producer, single-consumer queue of short MIDI messages. Never blocks or allocates.
pub struct MidiQueue {
    slots: [AtomicU32; CAP],
    write: AtomicUsize,
    read: AtomicUsize,
}

impl Default for MidiQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiQueue {
    pub const fn new() -> Self {
        Self { slots: [const { AtomicU32::new(0) }; CAP], write: AtomicUsize::new(0), read: AtomicUsize::new(0) }
    }

    pub fn push(&self, status: u8, d1: u8, d2: u8) {
        let i = self.write.fetch_add(1, Ordering::AcqRel) % CAP;
        let v = 0x8000_0000 | ((status as u32) << 16) | ((d1 as u32) << 8) | d2 as u32;
        self.slots[i].store(v, Ordering::Release);
    }

    /// Next message, if any. Call from one thread only.
    pub fn pop(&self) -> Option<[u8; 3]> {
        let w = self.write.load(Ordering::Acquire);
        let mut r = self.read.load(Ordering::Relaxed);
        if r == w {
            return None;
        }
        if w.wrapping_sub(r) > CAP {
            r = w.wrapping_sub(CAP); // overflowed: drop the oldest messages
        }
        let v = self.slots[r % CAP].swap(0, Ordering::AcqRel);
        if v == 0 {
            // A producer has claimed the slot but not stored yet; pick it up on the next call.
            self.read.store(r, Ordering::Relaxed);
            return None;
        }
        self.read.store(r.wrapping_add(1), Ordering::Relaxed);
        Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
    }
}

/// A decoded channel message. Values are normalised the same way nih-plug does it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Msg {
    NoteOn(u8, f32),
    NoteOff(u8),
    Cc(u8, f32),
    /// 0..1, 0.5 = centre.
    Bend(f32),
    Pressure(f32),
}

pub fn decode(m: [u8; 3]) -> Option<Msg> {
    let (d1, d2) = (m[1] & 0x7F, m[2] & 0x7F);
    match m[0] & 0xF0 {
        0x90 if d2 > 0 => Some(Msg::NoteOn(d1, d2 as f32 / 127.0)),
        0x90 | 0x80 => Some(Msg::NoteOff(d1)),
        0xB0 => Some(Msg::Cc(d1, d2 as f32 / 127.0)),
        0xD0 => Some(Msg::Pressure(d1 as f32 / 127.0)),
        0xE0 => Some(Msg::Bend((((d2 as u32) << 7) | d1 as u32) as f32 / 16383.0)),
        _ => None,
    }
}

/// Which of the 128 notes are currently held, written by the audio thread and drawn by the editor.
#[derive(Default)]
pub struct KeyState {
    bits: [AtomicU64; 2],
}

impl KeyState {
    pub fn set(&self, note: u8, on: bool) {
        let n = (note & 0x7F) as usize;
        let mask = 1u64 << (n % 64);
        if on {
            self.bits[n / 64].fetch_or(mask, Ordering::Relaxed);
        } else {
            self.bits[n / 64].fetch_and(!mask, Ordering::Relaxed);
        }
    }

    pub fn clear(&self) {
        self.bits[0].store(0, Ordering::Relaxed);
        self.bits[1].store(0, Ordering::Relaxed);
    }

    pub fn is_on(&self, note: u8) -> bool {
        let n = (note & 0x7F) as usize;
        self.bits[n / 64].load(Ordering::Relaxed) & (1u64 << (n % 64)) != 0
    }
}

/// MIDI from the standalone launcher's input ports. Unused (always empty) inside a DAW.
pub static EXTERNAL_MIDI: MidiQueue = MidiQueue::new();
/// Set by the standalone launcher; enables the computer-keyboard piano in the editor.
pub static STANDALONE: AtomicBool = AtomicBool::new(false);
/// Human-readable list of the connected MIDI inputs (standalone only).
pub static MIDI_STATUS: OnceLock<String> = OnceLock::new();

pub fn is_standalone() -> bool {
    STANDALONE.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_round_trips_in_order() {
        let q = MidiQueue::new();
        assert!(q.pop().is_none());
        q.push(0x90, 60, 100);
        q.push(0x80, 60, 0);
        assert_eq!(decode(q.pop().unwrap()), Some(Msg::NoteOn(60, 100.0 / 127.0)));
        assert_eq!(decode(q.pop().unwrap()), Some(Msg::NoteOff(60)));
        assert!(q.pop().is_none());
    }

    #[test]
    fn key_state_tracks_notes() {
        let k = KeyState::default();
        k.set(100, true);
        assert!(k.is_on(100) && !k.is_on(36));
        k.set(100, false);
        assert!(!k.is_on(100));
    }
}
