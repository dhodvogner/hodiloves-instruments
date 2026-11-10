//! Standalone launcher: opens Neon Break with its own audio output and listens to every connected
//! MIDI input, so it can be played from pads or a keyboard without a DAW.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use neonbreak::NeonBreak;
use nih_plug::prelude::*;
use plugin_common::standalone;

fn main() {
    let _midi = standalone::connect_all_midi("Neon Break");
    if !standalone::run_with_fallbacks(|args| nih_export_standalone_with_args::<NeonBreak, _>(args)) {
        standalone::report_failure("Neon Break");
    }
}
