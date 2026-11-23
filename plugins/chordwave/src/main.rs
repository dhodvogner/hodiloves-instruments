//! Standalone launcher: opens Chordwave with its own audio output and listens to every connected
//! MIDI input, so it can be used as a sketchpad (and MIDI exporter) without a DAW.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use chordwave::Chordwave;
use nih_plug::prelude::*;
use plugin_common::standalone;

fn main() {
    let _midi = standalone::connect_all_midi("Chordwave");
    if !standalone::run_with_fallbacks(|args| nih_export_standalone_with_args::<Chordwave, _>(args)) {
        standalone::report_failure("Chordwave");
    }
}
