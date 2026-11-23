//! Standard MIDI File reading and writing (no dependencies).
//!
//! Export writes a format 1 file: a conductor track (tempo, time signature, key signature and one
//! marker per section) followed by one named track per part, so every DAW shows the parts
//! separately and picks up the tempo and key. Import reads any format 0 or 1 file.

use crate::project::Note;
use crate::theory::*;

pub const PPQ: u16 = 480;

pub struct TrackOut {
    pub name: String,
    pub channel: u8,
    pub notes: Vec<Note>,
}

pub struct SongInfo {
    pub bpm: f32,
    pub key: u8,
    pub scale: Scale,
    /// Section markers: (start in beats, name).
    pub markers: Vec<(f32, String)>,
}

fn vlq(out: &mut Vec<u8>, mut v: u32) {
    let mut buf = [0u8; 5];
    let mut n = 0;
    buf[0] = (v & 0x7F) as u8;
    v >>= 7;
    while v > 0 {
        n += 1;
        buf[n] = ((v & 0x7F) as u8) | 0x80;
        v >>= 7;
    }
    for i in (0..=n).rev() {
        out.push(buf[i]);
    }
}

fn ticks(beats: f32) -> u32 {
    (beats.max(0.0) * PPQ as f32).round() as u32
}

/// Key signature meta event data: (sharps (+) or flats (-), minor flag).
pub fn key_signature(key: u8, scale: Scale) -> (i8, u8) {
    let minor = matches!(scale, Scale::Minor | Scale::HarmonicMinor | Scale::MelodicMinor);
    // The signature is that of the major key sharing the same notes.
    let major = if minor { (key + 3) % 12 } else { (key as i32 - scale.def().parent_offset as i32).rem_euclid(12) as u8 };
    let sf = match major {
        0 => 0, 7 => 1, 2 => 2, 9 => 3, 4 => 4, 11 => 5, 6 => 6,
        5 => -1, 10 => -2, 3 => -3, 8 => -4, 1 => -5,
        _ => 0,
    };
    (sf, minor as u8)
}

fn track_chunk(events: Vec<(u32, u8, Vec<u8>)>) -> Vec<u8> {
    // events: (tick, order, bytes). Sort by tick, then order (note off 0, meta 1, note on 2).
    let mut events = events;
    events.sort_by_key(|e| (e.0, e.1));
    let mut body = Vec::new();
    let mut last = 0u32;
    for (t, _, bytes) in events {
        vlq(&mut body, t - last);
        last = t;
        body.extend(bytes);
    }
    vlq(&mut body, 0);
    body.extend([0xFF, 0x2F, 0x00]);
    let mut chunk = b"MTrk".to_vec();
    chunk.extend((body.len() as u32).to_be_bytes());
    chunk.extend(body);
    chunk
}

fn meta_text(kind: u8, text: &str) -> Vec<u8> {
    let bytes: Vec<u8> = text.bytes().collect();
    let mut v = vec![0xFF, kind];
    vlq(&mut v, bytes.len() as u32);
    v.extend(bytes);
    v
}

pub fn write_smf(info: &SongInfo, tracks: &[TrackOut]) -> Vec<u8> {
    let mut out = b"MThd".to_vec();
    out.extend(6u32.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend(((tracks.len() + 1) as u16).to_be_bytes());
    out.extend(PPQ.to_be_bytes());

    // Conductor track.
    let mut conductor: Vec<(u32, u8, Vec<u8>)> = Vec::new();
    conductor.push((0, 1, meta_text(0x03, "Chordwave")));
    let micros = (60_000_000.0 / info.bpm.clamp(20.0, 400.0)).round() as u32;
    conductor.push((0, 1, vec![0xFF, 0x51, 0x03, (micros >> 16) as u8, (micros >> 8) as u8, micros as u8]));
    conductor.push((0, 1, vec![0xFF, 0x58, 0x04, 4, 2, 24, 8]));
    let (sf, mi) = key_signature(info.key, info.scale);
    conductor.push((0, 1, vec![0xFF, 0x59, 0x02, sf as u8, mi]));
    for (beat, name) in &info.markers {
        conductor.push((ticks(*beat), 1, meta_text(0x06, name)));
    }
    out.extend(track_chunk(conductor));

    for t in tracks {
        let ch = t.channel & 0x0F;
        let mut ev: Vec<(u32, u8, Vec<u8>)> = vec![(0, 1, meta_text(0x03, &t.name))];
        for n in &t.notes {
            let on = ticks(n.start);
            let off = (on + ticks(n.len).max(1)).max(on + 1);
            ev.push((on, 2, vec![0x90 | ch, n.pitch & 0x7F, n.vel.clamp(1, 127)]));
            ev.push((off, 0, vec![0x80 | ch, n.pitch & 0x7F, 0]));
        }
        out.extend(track_chunk(ev));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct TrackIn {
    pub name: String,
    pub notes: Vec<Note>,
    pub drums: bool,
}

#[derive(Clone, Debug, Default)]
pub struct MidiIn {
    pub bpm: Option<f32>,
    pub tracks: Vec<TrackIn>,
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Result<u8, String> {
        let v = *self.d.get(self.p).ok_or("File ends too early")?;
        self.p += 1;
        Ok(v)
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.d.get(self.p..self.p + n).ok_or("File ends too early")?;
        self.p += n;
        Ok(s)
    }
    fn vlq(&mut self) -> Result<u32, String> {
        let mut v = 0u32;
        for _ in 0..4 {
            let b = self.u8()?;
            v = (v << 7) | (b & 0x7F) as u32;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err("Bad variable-length number".into())
    }
}

pub fn read_smf(data: &[u8]) -> Result<MidiIn, String> {
    let mut r = Reader { d: data, p: 0 };
    if r.take(4)? != b"MThd" {
        return Err("Not a MIDI file".into());
    }
    let hlen = u32::from_be_bytes(r.take(4)?.try_into().unwrap()) as usize;
    let header = r.take(hlen.max(6))?;
    let ntracks = u16::from_be_bytes([header[2], header[3]]) as usize;
    let division = u16::from_be_bytes([header[4], header[5]]);
    if division & 0x8000 != 0 {
        return Err("SMPTE time division is not supported".into());
    }
    let ppq = division.max(1) as f32;
    let mut out = MidiIn::default();

    for index in 0..ntracks {
        let Ok(tag) = r.take(4) else { break };
        let Ok(len) = r.take(4).map(|b| u32::from_be_bytes(b.try_into().unwrap()) as usize) else { break };
        let Ok(body) = r.take(len.min(data.len().saturating_sub(r.p))) else { break };
        if tag != b"MTrk" {
            continue;
        }
        let mut t = Reader { d: body, p: 0 };
        let (mut tick, mut status) = (0u64, 0u8);
        let mut name = format!("Track {}", index + 1);
        let mut open: Vec<(u8, u8, u64, u8)> = Vec::new(); // (channel, pitch, start tick, velocity)
        let mut notes: Vec<(u8, Note)> = Vec::new();
        while t.p < body.len() {
            let Ok(delta) = t.vlq() else { break };
            tick += delta as u64;
            let Ok(first) = t.u8() else { break };
            let byte = if first & 0x80 != 0 {
                status = first;
                first
            } else {
                t.p -= 1;
                status
            };
            let result: Result<(), String> = (|| {
                match byte {
                    0xFF => {
                        let kind = t.u8()?;
                        let len = t.vlq()? as usize;
                        let data = t.take(len)?;
                        match kind {
                            0x03 if !data.is_empty() => name = String::from_utf8_lossy(data).trim().to_string(),
                            0x51 if data.len() == 3 && out.bpm.is_none() => {
                                let micros = ((data[0] as u32) << 16) | ((data[1] as u32) << 8) | data[2] as u32;
                                if micros > 0 {
                                    out.bpm = Some(60_000_000.0 / micros as f32);
                                }
                            }
                            _ => {}
                        }
                    }
                    0xF0 | 0xF7 => {
                        let len = t.vlq()? as usize;
                        t.take(len)?;
                    }
                    b => {
                        let channel = b & 0x0F;
                        let two = !matches!(b & 0xF0, 0xC0 | 0xD0);
                        let d1 = t.u8()?;
                        let d2 = if two { t.u8()? } else { 0 };
                        match b & 0xF0 {
                            0x90 if d2 > 0 => open.push((channel, d1, tick, d2)),
                            0x80 | 0x90 => {
                                if let Some(i) = open.iter().position(|o| o.0 == channel && o.1 == d1) {
                                    let (_, pitch, start, vel) = open.remove(i);
                                    let len = (tick - start).max(1) as f32 / ppq;
                                    notes.push((channel, Note::new(start as f32 / ppq, len, pitch, vel)));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Ok(())
            })();
            if result.is_err() {
                break;
            }
        }
        if !notes.is_empty() {
            let drums = notes.iter().all(|(c, _)| *c == 9);
            let mut list: Vec<Note> = notes.into_iter().map(|(_, n)| n).collect();
            list.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
            out.tracks.push(TrackIn { name, notes: list, drums });
        }
    }
    if out.tracks.is_empty() {
        return Err("The file has no notes".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> SongInfo {
        SongInfo { bpm: 174.0, key: 9, scale: Scale::Minor, markers: vec![(0.0, "Verse".into()), (16.0, "Chorus".into())] }
    }

    #[test]
    fn vlq_matches_the_spec_examples() {
        let enc = |v| {
            let mut o = Vec::new();
            vlq(&mut o, v);
            o
        };
        assert_eq!(enc(0), [0x00]);
        assert_eq!(enc(0x7F), [0x7F]);
        assert_eq!(enc(0x80), [0x81, 0x00]);
        assert_eq!(enc(0x3FFF), [0xFF, 0x7F]);
        assert_eq!(enc(0x1FFFFF), [0xFF, 0xFF, 0x7F]);
    }

    #[test]
    fn round_trip_keeps_notes_names_and_tempo() {
        let chords = TrackOut {
            name: "Chords".into(),
            channel: 0,
            notes: vec![Note::new(0.0, 4.0, 57, 100), Note::new(0.0, 4.0, 60, 100), Note::new(4.0, 2.5, 53, 90)],
        };
        let bass = TrackOut { name: "Bass".into(), channel: 1, notes: vec![Note::new(0.25, 0.5, 33, 110)] };
        let bytes = write_smf(&info(), &[chords, bass]);
        assert_eq!(&bytes[0..4], b"MThd");
        let back = read_smf(&bytes).unwrap();
        assert!((back.bpm.unwrap() - 174.0).abs() < 0.1);
        assert_eq!(back.tracks.len(), 2);
        assert_eq!(back.tracks[0].name, "Chords");
        assert_eq!(back.tracks[0].notes.len(), 3);
        assert_eq!(back.tracks[0].notes[2], Note::new(4.0, 2.5, 53, 90));
        assert_eq!(back.tracks[1].name, "Bass");
        assert_eq!(back.tracks[1].notes[0], Note::new(0.25, 0.5, 33, 110));
        assert!(!back.tracks[1].drums);
    }

    #[test]
    fn key_signatures() {
        assert_eq!(key_signature(0, Scale::Major), (0, 0));
        assert_eq!(key_signature(9, Scale::Minor), (0, 1)); // A minor
        assert_eq!(key_signature(2, Scale::Minor), (-1, 1)); // D minor: one flat
        assert_eq!(key_signature(7, Scale::Major), (1, 0));
        assert_eq!(key_signature(10, Scale::Major), (-2, 0)); // Bb major
        assert_eq!(key_signature(2, Scale::Dorian), (0, 0)); // D dorian = C major notes
    }

    #[test]
    fn reader_rejects_garbage_and_handles_running_status() {
        assert!(read_smf(b"hello").is_err());
        // Hand-built file: one track, running status note on/off.
        let mut f = b"MThd".to_vec();
        f.extend(6u32.to_be_bytes());
        f.extend([0, 0, 0, 1, 0x01, 0xE0]); // format 0, 1 track, 480 ppq
        let body: Vec<u8> = vec![
            0x00, 0x90, 60, 100, // note on
            0x60, 60, 0,         // running status: note on vel 0 = off, after 96 ticks
            0x00, 0xFF, 0x2F, 0x00,
        ];
        f.extend(b"MTrk");
        f.extend((body.len() as u32).to_be_bytes());
        f.extend(body);
        let m = read_smf(&f).unwrap();
        assert_eq!(m.tracks[0].notes, vec![Note::new(0.0, 0.2, 60, 100)]);
    }

    #[test]
    fn drum_channel_is_flagged() {
        let t = TrackOut { name: "Drums".into(), channel: 9, notes: vec![Note::new(0.0, 0.25, 36, 100)] };
        let m = read_smf(&write_smf(&info(), &[t])).unwrap();
        assert!(m.tracks[0].drums);
    }
}
