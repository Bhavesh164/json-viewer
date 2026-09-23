//! App settings — Linux port of `Sources/JSONViewerCore/AppSettings.swift`
//! and `windows/src/settings.rs`.
//!
//! Persisted as JSON in `$XDG_CONFIG_HOME/JSONViewer/config.json`
//! (fallback `~/.config/JSONViewer/config.json`), the same XDG pattern as
//! `mouseless/linux/src/settings.rs`.

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
    #[serde(default)]
    pub natural_scrolling: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            indent_spaces: 2,
            sort_keys_alphabetically: false,
            escape_slashes_in_stringify: true,
            auto_unwrap_stringified: true,
            default_tab: "Text".to_string(),
            font_size: 13.0,
            wrap_lines: true,
            natural_scrolling: false,
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

fn config_path() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return std::path::PathBuf::from(xdg).join(APP_NAME).join("config.json");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config").join(APP_NAME).join("config.json")
}

impl SettingsStore {
    pub fn new() -> Self {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let settings = std::fs::read(&path)
            .ok()
            .and_then(|data| serde_json::from_slice::<Settings>(&data).ok())
            .map(|s| s.clamped())
            .unwrap_or_default();
        Self { settings, path }
    }

    pub fn save(&self) {
        if let Ok(data) = serde_json::to_vec_pretty(&self.settings) {
            let _ = std::fs::write(&self.path, data);
        }
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}
