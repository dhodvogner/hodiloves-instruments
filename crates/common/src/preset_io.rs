//! Saving and loading user presets as small JSON files.
//!
//! A preset file stores every parameter by its id and plain value (Hz, ms, 0..1, enum index...),
//! the same convention the factory presets use, so the files are readable and can be edited by
//! hand:
//!
//! ```json
//! { "plugin": "Remedy", "name": "My Lead", "params": { "flt_cut": 1200.0, "a_uni": 5.0 } }
//! ```
//!
//! File dialogs run on their own thread so the host's window never freezes; a loaded preset is
//! handed back to the editor through [`PresetIo::take_loaded`] and applied there.

use nih_plug::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A preset read from disk, waiting to be applied by the editor.
pub struct Loaded {
    pub name: String,
    pub values: Vec<(String, f32)>,
    /// Anything else the plugin stored in the file (patterns, sample paths...).
    pub extra: Option<serde_json::Value>,
}

#[derive(Default)]
pub struct PresetIo {
    loaded: Arc<Mutex<Option<Loaded>>>,
    /// Short message for the editor ("Saved My Lead", "Not a Remedy preset"...).
    status: Arc<Mutex<Option<String>>>,
    /// Name of the user preset that is currently loaded, if any.
    pub current: Mutex<Option<String>>,
}

fn dialog(plugin: &str) -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter(format!("{plugin} preset"), &["json"])
}

fn to_json(plugin: &str, name: &str, values: &[(String, f32)], extra: Option<&serde_json::Value>) -> String {
    let params: serde_json::Map<String, serde_json::Value> =
        values.iter().map(|(k, v)| (k.clone(), serde_json::json!(v))).collect();
    let mut doc = serde_json::json!({ "plugin": plugin, "name": name, "params": params });
    if let Some(extra) = extra {
        doc["extra"] = extra.clone();
    }
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

fn from_json(plugin: &str, text: &str, fallback_name: &str) -> Result<Loaded, String> {
    let doc: serde_json::Value = serde_json::from_str(text).map_err(|_| "Not a preset file".to_string())?;
    let owner = doc.get("plugin").and_then(|v| v.as_str()).unwrap_or("");
    if owner != plugin {
        return Err(if owner.is_empty() { "Not a preset file".into() } else { format!("That is a {owner} preset") });
    }
    let params = doc.get("params").and_then(|v| v.as_object()).ok_or("Preset has no parameters")?;
    let values = params.iter().filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f as f32))).collect();
    let name = doc.get("name").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(fallback_name);
    Ok(Loaded { name: name.to_string(), values, extra: doc.get("extra").cloned() })
}

/// Ask for a file with one of `extensions` on a background thread and hand the path to `on_pick`.
pub fn pick_file(label: &str, extensions: &[&str], on_pick: impl FnOnce(PathBuf) + Send + 'static) {
    let dialog = rfd::FileDialog::new().add_filter(label, extensions);
    std::thread::spawn(move || {
        if let Some(path) = dialog.pick_file() {
            on_pick(path);
        }
    });
}

fn stem(path: &PathBuf) -> String {
    path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Preset".into())
}

impl PresetIo {
    /// Ask where to save, then write every current parameter value. `suggested` pre-fills the name.
    pub fn export(&self, plugin: &'static str, suggested: &str, params: &dyn Params) {
        self.export_with(plugin, suggested, Some(params), None);
    }

    /// Like [`Self::export`], with optional extra data stored next to the parameters. Pass no
    /// `params` for files that hold only extra data (a single drum pattern, for example).
    pub fn export_with(
        &self,
        plugin: &'static str,
        suggested: &str,
        params: Option<&dyn Params>,
        extra: Option<serde_json::Value>,
    ) {
        let values: Vec<(String, f32)> = params
            .map(|p| p.param_map())
            .unwrap_or_default()
            .into_iter()
            // SAFETY: the pointers come from `params`, which is alive for this call.
            .map(|(id, ptr, _)| (id, unsafe { ptr.unmodulated_plain_value() }))
            .collect();
        let status = self.status.clone();
        let suggested = suggested.to_string();
        std::thread::spawn(move || {
            let Some(mut path) = dialog(plugin).set_file_name(format!("{suggested}.json")).save_file() else {
                return;
            };
            if path.extension().is_none() {
                path.set_extension("json");
            }
            let name = stem(&path);
            let message = match std::fs::write(&path, to_json(plugin, &name, &values, extra.as_ref())) {
                Ok(()) => format!("Saved \"{name}\""),
                Err(err) => format!("Could not save: {err}"),
            };
            *status.lock().unwrap() = Some(message);
        });
    }

    /// Ask for a preset file and read it. The editor picks the result up with [`Self::take_loaded`].
    pub fn import(&self, plugin: &'static str) {
        let (loaded, status) = (self.loaded.clone(), self.status.clone());
        std::thread::spawn(move || {
            let Some(path) = dialog(plugin).pick_file() else { return };
            let result = std::fs::read_to_string(&path)
                .map_err(|err| format!("Could not open: {err}"))
                .and_then(|text| from_json(plugin, &text, &stem(&path)));
            match result {
                Ok(preset) => *loaded.lock().unwrap() = Some(preset),
                Err(message) => *status.lock().unwrap() = Some(message),
            }
        });
    }

    pub fn take_loaded(&self) -> Option<Loaded> {
        self.loaded.lock().ok()?.take()
    }

    pub fn take_status(&self) -> Option<String> {
        self.status.lock().ok()?.take()
    }
}

/// Reset every parameter to its default, then apply `values` (id, plain value) on top.
/// Unknown ids are ignored, so presets survive parameters being added or removed.
pub fn apply_values(params: &dyn Params, setter: &ParamSetter, values: &[(String, f32)]) {
    for (id, ptr, _group) in params.param_map() {
        // SAFETY: the pointers come from `params`, which outlives this call.
        unsafe {
            let normalized = match values.iter().find(|(k, _)| *k == id) {
                Some((_, plain)) => ptr.preview_normalized(*plain),
                None => ptr.default_normalized_value(),
            };
            setter.raw_context.raw_begin_set_parameter(ptr);
            setter.raw_context.raw_set_parameter_normalized(ptr, normalized);
            setter.raw_context.raw_end_set_parameter(ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trips_and_rejects_other_plugins() {
        let values = vec![("flt_cut".to_string(), 1200.0), ("a_uni".to_string(), 5.0)];
        let text = to_json("Remedy", "My Lead", &values, Some(&serde_json::json!({ "k": [1, 2] })));
        let back = from_json("Remedy", &text, "x").unwrap();
        assert_eq!(back.name, "My Lead");
        assert_eq!(back.values.len(), 2);
        assert_eq!(back.extra.unwrap()["k"][1], 2);
        assert!(back.values.iter().any(|(k, v)| k == "flt_cut" && (*v - 1200.0).abs() < 1e-3));
        assert!(from_json("Wub Machine", &text, "x").is_err());
        assert!(from_json("Remedy", "not json", "x").is_err());
    }
}
