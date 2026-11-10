//! Audio samples: every drum can play a WAV file instead of (or layered with) its synthesized
//! sound. The file paths are saved with the project; the audio itself is re-read from disk.

use crate::params::VOICES;
use nih_plug::params::persist::PersistentField;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Longest sample that is kept, in seconds; anything after that is cut off.
const MAX_SECONDS: usize = 12;

pub struct Sample {
    pub l: Vec<f32>,
    pub r: Vec<f32>,
    pub rate: f32,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Decode a WAV file: 8 / 16 / 24 / 32-bit PCM or 32 / 64-bit float, mono or stereo (extra
/// channels are ignored).
pub fn decode_wav(bytes: &[u8]) -> Result<Sample, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("Not a WAV file".into());
    }
    let (mut format, mut channels, mut rate, mut bits) = (0u16, 0usize, 0u32, 0usize);
    let mut data: Option<&[u8]> = None;
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32_at(bytes, pos + 4).unwrap_or(0) as usize;
        let body = pos + 8;
        let end = (body + size).min(bytes.len());
        if id == b"fmt " {
            format = u16_at(bytes, body).ok_or("Broken WAV header")?;
            channels = u16_at(bytes, body + 2).ok_or("Broken WAV header")? as usize;
            rate = u32_at(bytes, body + 4).ok_or("Broken WAV header")?;
            bits = u16_at(bytes, body + 14).ok_or("Broken WAV header")? as usize;
            if format == 0xFFFE {
                // WAVE_FORMAT_EXTENSIBLE: the real format is the first word of the sub-format.
                format = u16_at(bytes, body + 24).ok_or("Broken WAV header")?;
            }
        } else if id == b"data" {
            data = Some(&bytes[body..end]);
        }
        pos = body + size + (size & 1);
    }
    let data = data.ok_or("WAV file has no audio")?;
    // Vorbis-in-WAV (type 0x674F and friends) carries a plain Ogg Vorbis stream in the data chunk.
    if data.starts_with(b"OggS") {
        return decode_ogg(data);
    }
    let width = bits / 8;
    if channels == 0 || rate == 0 || !matches!((format, bits), (1, 8 | 16 | 24 | 32) | (3, 32 | 64)) {
        let kind = match format {
            2 => "MS ADPCM",
            0x11 => "IMA ADPCM",
            0x55 => "MP3",
            0x674F | 0x6750 | 0x6751 | 0x676F | 0x6770 | 0x6771 => "Ogg Vorbis",
            _ => "compressed",
        };
        return Err(format!("This WAV is {kind} encoded (type {format}). Convert it to a normal PCM WAV (16 or 24 bit) first."));
    }
    let frame = width * channels;
    let frames = (data.len() / frame).min(rate as usize * MAX_SECONDS);
    let read = |at: usize| -> f32 {
        let s = &data[at..at + width];
        match (format, bits) {
            (1, 8) => (s[0] as f32 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f32 / 32_768.0,
            (1, 24) => (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f32 / 8_388_608.0,
            (1, 32) => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
            (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            _ => f64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]) as f32,
        }
    };
    let clean = |x: f32| if x.is_finite() { x.clamp(-4.0, 4.0) } else { 0.0 };
    let (mut l, mut r) = (Vec::with_capacity(frames), Vec::with_capacity(frames));
    for f in 0..frames {
        let left = clean(read(f * frame));
        l.push(left);
        r.push(if channels > 1 { clean(read(f * frame + width)) } else { left });
    }
    if frames == 0 {
        return Err("WAV file is empty".into());
    }
    Ok(Sample { l, r, rate: rate as f32 })
}

/// Decode an Ogg Vorbis stream (mono or stereo; extra channels are ignored).
pub fn decode_ogg(bytes: &[u8]) -> Result<Sample, String> {
    use lewton::inside_ogg::OggStreamReader;
    let mut reader = OggStreamReader::new(std::io::Cursor::new(bytes)).map_err(|err| format!("Could not read the Ogg Vorbis audio: {err}"))?;
    let channels = reader.ident_hdr.audio_channels as usize;
    let rate = reader.ident_hdr.audio_sample_rate;
    if channels == 0 || rate == 0 {
        return Err("Broken Ogg Vorbis header".into());
    }
    let max = rate as usize * MAX_SECONDS;
    let (mut l, mut r) = (Vec::new(), Vec::new());
    while l.len() < max {
        match reader.read_dec_packet_itl() {
            Ok(Some(packet)) => {
                for frame in packet.chunks_exact(channels) {
                    let left = frame[0] as f32 / 32_768.0;
                    l.push(left);
                    r.push(if channels > 1 { frame[1] as f32 / 32_768.0 } else { left });
                }
            }
            Ok(None) => break,
            // Keep what was decoded if the tail of the file is damaged.
            Err(_) if !l.is_empty() => break,
            Err(err) => return Err(format!("Could not decode the Ogg Vorbis audio: {err}")),
        }
    }
    l.truncate(max);
    r.truncate(max);
    if l.is_empty() {
        return Err("Sample is empty".into());
    }
    Ok(Sample { l, r, rate: rate as f32 })
}

/// Decode a WavPack (.wv) file: lossless 16 / 24 / 32-bit, mono or stereo.
pub fn decode_wavpack(bytes: &[u8]) -> Result<Sample, String> {
    let stream = wavicle::decode_stream(bytes).map_err(|err| format!("Could not decode the WavPack file: {err}"))?;
    let channels = stream.channels as usize;
    if channels == 0 || stream.sample_rate == 0 {
        return Err("Broken WavPack header".into());
    }
    let scale = 1.0 / (1u64 << (stream.bits_per_sample.clamp(8, 32) - 1)) as f32;
    let max = stream.sample_rate as usize * MAX_SECONDS;
    let value = |s: i32| {
        let x = if stream.is_float { f32::from_bits(s as u32) } else { s as f32 * scale };
        if x.is_finite() { x.clamp(-4.0, 4.0) } else { 0.0 }
    };
    let (mut l, mut r) = (Vec::new(), Vec::new());
    for frame in stream.samples.chunks_exact(channels).take(max) {
        let left = value(frame[0]);
        l.push(left);
        r.push(if channels > 1 { value(frame[1]) } else { left });
    }
    if l.is_empty() {
        return Err("Sample is empty".into());
    }
    Ok(Sample { l, r, rate: stream.sample_rate as f32 })
}

/// Read an audio file, recognised by its contents: WAV, Ogg Vorbis or WavPack.
pub fn load(path: &str) -> Result<Sample, String> {
    let bytes = std::fs::read(path).map_err(|err| format!("Could not open sample: {err}"))?;
    if bytes.starts_with(b"OggS") {
        decode_ogg(&bytes)
    } else if bytes.starts_with(b"wvpk") {
        decode_wavpack(&bytes)
    } else if bytes.starts_with(b"RIFF") {
        decode_wav(&bytes)
    } else {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        Err(format!("\"{name}\" is not a supported audio file. Supported: WAV, Ogg Vorbis (.ogg), WavPack (.wv)."))
    }
}

/// File name without folder and extension, for display.
pub fn display_name(path: &str) -> String {
    std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------
// Shared sample slots
// ---------------------------------------------------------------------------------------------

/// One sample slot per drum, shared between the editor (loads files), the host (saves and restores
/// the paths) and the audio thread (picks up new audio whenever `version` changes).
pub struct SampleStore {
    paths: Mutex<Vec<String>>,
    loaded: [Mutex<Option<Arc<Sample>>>; VOICES],
    version: AtomicU32,
    /// Paths were restored from a project and still have to be read from disk.
    pending: AtomicBool,
    status: Mutex<Option<String>>,
}

/// Cheaply clonable handle to the sample slots, so background loaders can hold on to them.
#[derive(Clone, Default)]
pub struct SampleCell(Arc<SampleStore>);

impl std::ops::Deref for SampleCell {
    type Target = SampleStore;

    fn deref(&self) -> &SampleStore {
        &self.0
    }
}

impl Default for SampleStore {
    fn default() -> Self {
        Self {
            paths: Mutex::new(vec![String::new(); VOICES]),
            loaded: std::array::from_fn(|_| Mutex::new(None)),
            version: AtomicU32::new(1),
            pending: AtomicBool::new(false),
            status: Mutex::new(None),
        }
    }
}

impl SampleStore {
    pub fn version(&self) -> u32 {
        self.version.load(Ordering::Acquire)
    }

    pub fn paths(&self) -> Vec<String> {
        self.paths.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Display name of the sample on `voice`, if one is set.
    pub fn name(&self, voice: usize) -> Option<String> {
        let paths = self.paths.lock().ok()?;
        let path = paths.get(voice)?;
        (!path.is_empty()).then(|| display_name(path))
    }

    pub fn take_status(&self) -> Option<String> {
        self.status.lock().ok()?.take()
    }

    /// Read `path` from disk and put it on `voice`; an empty path removes the sample. Blocks on
    /// file access, so call it from a background thread.
    pub fn set(&self, voice: usize, path: &str) {
        let result = if path.is_empty() { Ok(None) } else { load(path).map(|s| Some(Arc::new(s))) };
        match result {
            Ok(sample) => {
                if let Ok(mut paths) = self.paths.lock() {
                    paths[voice] = path.to_string();
                }
                if let Ok(mut slot) = self.loaded[voice].lock() {
                    *slot = sample;
                }
                self.version.fetch_add(1, Ordering::AcqRel);
            }
            Err(message) => {
                if let Ok(mut status) = self.status.lock() {
                    *status = Some(message);
                }
            }
        }
    }

    /// Replace every path (from a project or preset file) and mark them for loading.
    pub fn set_paths(&self, new_paths: &[String]) {
        if let Ok(mut paths) = self.paths.lock() {
            for (i, p) in paths.iter_mut().enumerate() {
                *p = new_paths.get(i).cloned().unwrap_or_default();
            }
        }
        self.pending.store(true, Ordering::Release);
    }

    /// True once after paths were restored; the caller should then run [`Self::load_all`].
    pub fn take_pending(&self) -> bool {
        self.pending.swap(false, Ordering::AcqRel)
    }

    /// Read every path from disk. A file that has gone missing leaves its drum on the synth.
    pub fn load_all(&self) {
        let paths = self.paths();
        let mut missing = 0;
        for (voice, path) in paths.iter().enumerate().take(VOICES) {
            let sample = if path.is_empty() { None } else { load(path).ok().map(Arc::new) };
            missing += (!path.is_empty() && sample.is_none()) as usize;
            if let Ok(mut slot) = self.loaded[voice].lock() {
                *slot = sample;
            }
        }
        if missing > 0 {
            if let Ok(mut status) = self.status.lock() {
                *status = Some(format!("{missing} sample file(s) could not be found"));
            }
        }
        self.version.fetch_add(1, Ordering::AcqRel);
    }

    /// For the audio thread: the current sample of `voice`, or `Err` if it is being swapped right
    /// now (try again on the next block).
    pub fn try_get(&self, voice: usize) -> Result<Option<Arc<Sample>>, ()> {
        self.loaded[voice].try_lock().map(|s| s.clone()).map_err(|_| ())
    }
}

impl<'a> PersistentField<'a, Vec<String>> for SampleCell {
    fn set(&self, new_value: Vec<String>) {
        self.set_paths(&new_value);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Vec<String>) -> R,
    {
        f(&self.paths())
    }
}

// ---------------------------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------------------------

/// Plays one sample, one hit at a time, with tuning and a fade set by the DECAY knob.
#[derive(Default)]
pub struct Player {
    sample: Option<Arc<Sample>>,
    on: bool,
    pos: f64,
    inc: f64,
    vel: f32,
    amp: f32,
    k_amp: f32,
    /// Fading remainder of an interrupted hit, so retriggers do not click.
    declick: (f32, f32),
    last: (f32, f32),
}

impl Player {
    pub fn set_sample(&mut self, sample: Option<Arc<Sample>>) {
        self.sample = sample;
        self.on = false;
    }

    pub fn has_sample(&self) -> bool {
        self.sample.is_some()
    }

    pub fn is_on(&self) -> bool {
        self.on || self.declick.0.abs() > 1e-5 || self.declick.1.abs() > 1e-5
    }

    pub fn reset(&mut self) {
        self.on = false;
        self.declick = (0.0, 0.0);
        self.last = (0.0, 0.0);
    }

    /// `tune` in semitones, `decay` 0..1 (at the top the sample plays to its end untouched).
    pub fn trigger(&mut self, vel: f32, tune: f32, decay: f32, sample_rate: f32) {
        let Some(sample) = &self.sample else { return };
        self.declick.0 += self.last.0;
        self.declick.1 += self.last.1;
        self.on = true;
        self.pos = 0.0;
        self.inc = (sample.rate / sample_rate) as f64 * (tune as f64 / 12.0).exp2();
        self.vel = vel.clamp(0.0, 1.0);
        self.amp = 1.0;
        self.k_amp = if decay >= 0.95 { 1.0 } else { (-1.0 / ((0.03 + 1.5 * decay * decay) * sample_rate)).exp() };
    }

    pub fn choke(&mut self, sample_rate: f32) {
        self.k_amp = self.k_amp.min((-1.0 / (0.006 * sample_rate)).exp());
    }

    #[inline]
    pub fn process(&mut self) -> (f32, f32) {
        let tail = self.declick;
        if tail.0 != 0.0 || tail.1 != 0.0 {
            let fade = |x: f32| if x.abs() < 1e-5 { 0.0 } else { x * 0.985 };
            self.declick = (fade(tail.0), fade(tail.1));
        }
        let mut out = (0.0, 0.0);
        if self.on {
            if let Some(sample) = &self.sample {
                let i = self.pos as usize;
                if i + 1 < sample.l.len() && self.amp > 1e-4 {
                    let t = (self.pos - i as f64) as f32;
                    let g = self.amp * self.vel;
                    out = (
                        (sample.l[i] + (sample.l[i + 1] - sample.l[i]) * t) * g,
                        (sample.r[i] + (sample.r[i + 1] - sample.r[i]) * t) * g,
                    );
                    self.pos += self.inc;
                    self.amp *= self.k_amp;
                } else {
                    self.on = false;
                }
            } else {
                self.on = false;
            }
        }
        self.last = out;
        (out.0 + tail.0, out.1 + tail.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-bit stereo WAV: left ramps up, right is silent.
    fn wav(frames: usize, rate: u32) -> Vec<u8> {
        let data_len = (frames * 4) as u32;
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + data_len).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(rate.to_le_bytes());
        b.extend((rate * 4).to_le_bytes());
        b.extend(4u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend(data_len.to_le_bytes());
        for f in 0..frames {
            b.extend(((f * 16_000 / frames) as i16).to_le_bytes());
            b.extend(0i16.to_le_bytes());
        }
        b
    }

    #[test]
    fn decodes_pcm_wav_and_rejects_garbage() {
        let s = decode_wav(&wav(1000, 44_100)).unwrap();
        assert_eq!((s.l.len(), s.r.len(), s.rate), (1000, 1000, 44_100.0));
        assert!(s.l[999] > 0.45 && s.l[999] < 0.5 && s.r.iter().all(|x| *x == 0.0));
        assert!(decode_wav(b"hello").is_err());
        assert!(decode_wav(&wav(0, 44_100)).is_err());
        let mut truncated = wav(1000, 44_100);
        truncated.truncate(500);
        assert!(decode_wav(&truncated).is_ok(), "a cut-off file should still give what is there");
    }

    #[test]
    fn player_follows_tune_and_decay_and_stops_at_the_end() {
        let sample = Arc::new(decode_wav(&wav(4800, 48_000)).unwrap());
        let mut p = Player::default();
        p.trigger(1.0, 0.0, 1.0, 48_000.0);
        assert_eq!(p.process(), (0.0, 0.0), "no sample loaded: silent");

        p.set_sample(Some(sample));
        // An octave up plays twice as fast: the ramp ends after half the frames.
        p.trigger(1.0, 12.0, 1.0, 48_000.0);
        let out: Vec<f32> = (0..4800).map(|_| p.process().0).collect();
        assert!(out[2390] > 0.45, "ramp should be near its top at the halfway point: {}", out[2390]);
        assert!(out[2500..].iter().all(|x| x.abs() < 0.01), "sample should have ended");
        assert!(!p.is_on());

        // A short decay fades the ramp out before it gets loud.
        p.trigger(1.0, 0.0, 0.0, 48_000.0);
        let peak = (0..4800).map(|_| p.process().0).fold(0.0f32, f32::max);
        assert!(peak < 0.2, "decay did not fade the sample: {peak}");
    }
}
