#![windows_subsystem = "windows"]

mod app;
mod clipboard;
mod json;
mod model;
mod python;
mod settings;
mod settings_dialog;
mod util;

use std::sync::{Arc, Mutex};

fn main() {
    let store = settings::SettingsStore::new();
    let settings = Arc::new(Mutex::new(store.settings.clone()));
    let store = Arc::new(Mutex::new(store));
    // Optional file path: "jsonviewer.exe document.json" (also used by
    // "Open with" / shell associations when a file is double-clicked).
    let initial_file = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|p| p.is_file());
    app::run(settings, store, initial_file);
}
