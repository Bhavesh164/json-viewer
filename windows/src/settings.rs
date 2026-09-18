//! App settings — Rust port of `Sources/JSONViewerCore/AppSettings.swift`.
//! Persisted as JSON in `%APPDATA%\JSONViewer\config.json`
//! (same pattern as `mouseless/windows/src/settings.rs`).

use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "JSONViewer";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub indent_spaces: i32,
    pub sort_keys_alphabetically: bool,
    pub escape_slashes_in_stringify: bool,
    pub auto_unwrap_stringified: bool,
    pub default_tab: String,
    pub font_size: f64,
    pub wrap_lines: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            indent_spaces: 2,
            sort_keys_alphabetically: false,
            escape_slashes_in_stringify: true,
            auto_unwrap_stringified: true,
            default_tab: "Text".to_string(),
            font_size: 12.0,
            wrap_lines: false,
        }
    }
}

impl Settings {
    pub fn clamped(&self) -> Settings {
        let mut c = self.clone();
        if c.indent_spaces != 2 && c.indent_spaces != 4 && c.indent_spaces != -1 {
            c.indent_spaces = 2;
        }
        if c.default_tab != "Text" && c.default_tab != "Viewer" && c.default_tab != "Split" {
            c.default_tab = "Text".to_string();
        }
        c.font_size = c.font_size.clamp(9.0, 24.0);
        c
    }

    pub fn reset_to_defaults(&mut self) {
        *self = Settings::default();
    }

    pub fn indent_label(&self) -> String {
        match self.indent_spaces {
            4 => "4 Spaces".to_string(),
            -1 => "Tabs".to_string(),
            _ => "2 Spaces (Default)".to_string(),
        }
    }
}

pub struct SettingsStore {
    pub settings: Settings,
    path: std::path::PathBuf,
}

impl SettingsStore {
    pub fn new() -> Self {
        let mut dir = if let Ok(appdata) = std::env::var("APPDATA") {
            std::path::PathBuf::from(appdata)
        } else {
            std::path::PathBuf::from(".")
        };
        dir.push(APP_NAME);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.json");
        let settings = if let Ok(data) = std::fs::read(&path) {
            serde_json::from_slice::<Settings>(&data)
                .map(|s| s.clamped())
                .unwrap_or_default()
        } else {
            Settings::default()
        };
        Self { settings, path }
    }

    pub fn save(&self) {
        if let Ok(data) = serde_json::to_vec_pretty(&self.settings) {
            let _ = std::fs::write(&self.path, data);
        }
    }
}
