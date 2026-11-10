//! Helpers for the standalone launchers: open every MIDI input and pick a working audio setup.

use crate::midi::{EXTERNAL_MIDI, MIDI_STATUS, STANDALONE};
use midir::{Ignore, MidiInput, MidiInputConnection};
use std::sync::atomic::Ordering;

/// Connect to every MIDI input port on the machine and forward its channel messages into
/// [`EXTERNAL_MIDI`]. Keep the returned connections alive for as long as the app runs.
pub fn connect_all_midi(client: &str) -> Vec<MidiInputConnection<()>> {
    STANDALONE.store(true, Ordering::Relaxed);
    let mut conns = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let Ok(probe) = MidiInput::new(client) else {
        let _ = MIDI_STATUS.set("MIDI unavailable".into());
        return conns;
    };
    for port in probe.ports() {
        let name = probe.port_name(&port).unwrap_or_else(|_| "MIDI input".into());
        let Ok(mut input) = MidiInput::new(client) else { continue };
        input.ignore(Ignore::All);
        let result = input.connect(
            &port,
            "in",
            |_stamp, msg, _| {
                if msg.len() >= 2 && msg[0] >= 0x80 && msg[0] < 0xF0 {
                    EXTERNAL_MIDI.push(msg[0], msg[1], msg.get(2).copied().unwrap_or(0));
                }
            },
            (),
        );
        if let Ok(conn) = result {
            eprintln!("MIDI input connected: {name}");
            names.push(name);
            conns.push(conn);
        }
    }
    let status = if names.is_empty() {
        "No MIDI device - use the on-screen or computer keyboard".to_string()
    } else {
        format!("MIDI: {}", names.join(", "))
    };
    let _ = MIDI_STATUS.set(status);
    conns
}

/// Largest block (in frames) the default output device hands to its callback when a buffer of
/// `requested` frames is asked for, plus the device's sample rate.
fn observe_block(requested: u32) -> Option<(u32, u32)> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::atomic::AtomicU32;
    use std::sync::Arc;

    let device = cpal::default_host().default_output_device()?;
    let rate = device.default_output_config().ok()?.sample_rate().0;
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(rate),
        buffer_size: cpal::BufferSize::Fixed(requested),
    };
    let seen = Arc::new(AtomicU32::new(0));
    let seen_cb = seen.clone();
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                data.fill(0.0);
                seen_cb.fetch_max((data.len() / 2) as u32, Ordering::Relaxed);
            },
            |_| {},
            None,
        )
        .ok()?;
    stream.play().ok()?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    drop(stream);
    match seen.load(Ordering::Relaxed) {
        0 => None,
        n => Some((rate, n)),
    }
}

/// Find a sample rate and period size the default output device really works with.
///
/// Windows (WASAPI shared mode) ignores the requested buffer size and delivers blocks of its own
/// size, and nih-plug's standalone wrapper stops its audio thread when a block is larger than the
/// configured period. So play silence briefly, see what the device delivers, and ask for that.
fn probe_audio() -> Option<(u32, u32)> {
    let mut period = 512;
    let mut rate = 48_000;
    for _ in 0..4 {
        let (r, seen) = observe_block(period)?;
        rate = r;
        if seen <= period {
            break;
        }
        period = seen;
    }
    Some((rate, period))
}

/// Run `start` with the user's own command line. If they gave no options, the audio settings are
/// matched to the output device first; if that still fails, the other common rates are tried.
pub fn run_with_fallbacks(start: impl Fn(Vec<String>) -> bool) -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return start(args);
    }
    if let Some((rate, period)) = probe_audio() {
        eprintln!("Audio output: {rate} Hz, {period} samples per block");
        let mut a = args.clone();
        a.extend(["--sample-rate".into(), rate.to_string(), "--period-size".into(), period.to_string()]);
        if start(a) {
            return true;
        }
    }
    if start(args.clone()) {
        return true;
    }
    for rate in ["44100", "96000", "88200", "192000"] {
        let mut a = args.clone();
        a.push("--sample-rate".into());
        a.push(rate.into());
        if start(a) {
            return true;
        }
    }
    false
}

/// Tell the user why nothing opened (the release builds have no console window).
pub fn report_failure(app: &str) {
    let text = format!(
        "{app} could not open an audio output.\n\nCheck that an output device is enabled in Windows sound settings. \
         You can also start it from a terminal with options such as:\n  --output-device \"name\"  --sample-rate 48000  --period-size 512"
    );
    eprintln!("{text}");
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(hwnd: *mut core::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
        }
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let (t, c) = (wide(&text), wide(app));
        // SAFETY: both strings are valid, NUL-terminated UTF-16 that outlive the call.
        unsafe {
            MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), 0x10);
        }
    }
}
