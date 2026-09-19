//! Desktop integration for Linux / Omarchy / Wayland / X11.
//!
//! Provides embedded icon data for eframe and self-registers the .desktop
//! file and application icons into `$HOME/.local/share/` so that Omarchy's
//! app launcher (`AppLibrary.qml`, `walker`, `rofi`) always resolves and
//! displays the JSON Viewer icon when installed in `/usr/local/bin`.

use std::fs;
use std::path::{Path, PathBuf};

pub const ICON_PNG_BYTES: &[u8] = include_bytes!("../resources/AppIcon.png");
pub const DESKTOP_ENTRY_STR: &str = include_str!("../resources/jsonviewer.desktop");

/// Returns the embedded icon for the eframe viewport builder.
pub fn app_icon() -> Option<egui::IconData> {
    eframe::icon_data::from_png_bytes(ICON_PNG_BYTES).ok()
}

/// Ensures that the `.desktop` file and icon files exist in standard user XDG directories.
/// This enables Omarchy's app launcher to find the icon and entry even if the user
/// simply copied the standalone binary into `/usr/local/bin` or `~/.local/bin`.
pub fn ensure_desktop_integration() {
    let home = match std::env::var("HOME") {
        Ok(h) if !h.is_empty() => PathBuf::from(h),
        _ => return,
    };

    let data_dir = match std::env::var("XDG_DATA_HOME") {
        Ok(d) if !d.is_empty() => PathBuf::from(d),
        _ => home.join(".local").join("share"),
    };

    let apps_dir = data_dir.join("applications");
    let desktop_file = apps_dir.join("jsonviewer.desktop");

    // Write desktop entry if missing or outdated
    let _ = fs::create_dir_all(&apps_dir);
    let should_write_desktop = match fs::read_to_string(&desktop_file) {
        Ok(existing) => existing != DESKTOP_ENTRY_STR,
        Err(_) => true,
    };
    if should_write_desktop {
        let _ = fs::write(&desktop_file, DESKTOP_ENTRY_STR);
        // Silently notify desktop database if available
        let _ = std::process::Command::new("update-desktop-database")
            .arg(&apps_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    // Install icon in standard XDG / Omarchy searched locations:
    // 1. ~/.local/share/icons/hicolor/512x512/apps/jsonviewer.png
    // 2. ~/.local/share/pixmaps/jsonviewer.png
    // 3. ~/.icons/jsonviewer.png (Omarchy explicitly searches ~/.icons)
    let icon_targets = [
        data_dir.join("icons").join("hicolor").join("512x512").join("apps").join("jsonviewer.png"),
        data_dir.join("icons").join("hicolor").join("256x256").join("apps").join("jsonviewer.png"),
        data_dir.join("icons").join("hicolor").join("128x128").join("apps").join("jsonviewer.png"),
        data_dir.join("pixmaps").join("jsonviewer.png"),
        home.join(".icons").join("jsonviewer.png"),
        home.join(".local").join("share").join("icons").join("jsonviewer.png"),
    ];

    for target in &icon_targets {
        install_icon_if_needed(target);
    }
}

fn install_icon_if_needed(path: &Path) {
    if path.is_file() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, ICON_PNG_BYTES);
}
