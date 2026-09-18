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
    app::run(settings, store);
}
