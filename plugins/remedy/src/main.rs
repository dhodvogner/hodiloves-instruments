//! Standalone launcher: opens Remedy with its own audio output and listens to every connected
//! MIDI input, so it can be played from a keyboard without a DAW.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nih_plug::prelude::*;
use plugin_common::standalone;
use remedy::Remedy;

fn main() {
    let _midi = standalone::connect_all_midi("Remedy");
    if !standalone::run_with_fallbacks(|args| nih_export_standalone_with_args::<Remedy, _>(args)) {
        standalone::report_failure("Remedy");
    }
}
