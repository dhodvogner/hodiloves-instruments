//! Standalone launcher: opens the Wub Machine with its own audio output and listens to every
//! connected MIDI input, so it can be played from a keyboard without a DAW.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use nih_plug::prelude::*;
use plugin_common::standalone;
use wubmachine::WubMachine;

fn main() {
    let _midi = standalone::connect_all_midi("Wub Machine");
    if !standalone::run_with_fallbacks(|args| nih_export_standalone_with_args::<WubMachine, _>(args)) {
        standalone::report_failure("Wub Machine");
    }
}
