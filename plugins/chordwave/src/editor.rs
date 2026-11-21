//! egui editor (placeholder while the UI is built).
use crate::params::ChordwaveParams;
use crate::Shared;
use nih_plug::prelude::*;
use nih_plug_egui::create_egui_editor;
use std::sync::Arc;

pub fn create(params: Arc<ChordwaveParams>, _shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    create_egui_editor(params.editor_state.clone(), (), |_, _| {}, move |_, _, _| {})
}
