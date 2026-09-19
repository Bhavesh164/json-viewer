pub mod app;
pub mod clipboard;
pub mod desktop;
pub mod json;
pub mod model;
pub mod python;
pub mod settings;

pub use app::ViewerApp;
pub use json::JSONParser;
pub use model::{AppTab, DocumentModel};
pub use python::parse_python_literal;
pub use settings::{Settings, SettingsStore};

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Candidate monospace fonts (e.g. JetBrainsMono Nerd Font on Arch / Omarchy)
    let candidate_monos = [
        "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf",
        "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    ];

    // Candidate proportional fonts (e.g. Noto Sans on Arch / Omarchy)
    let candidate_props = [
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/TTF/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ];

    for path in candidate_monos {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "system_mono".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                family.insert(0, "system_mono".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.push("system_mono".to_owned());
            }
            break;
        }
    }

    for path in candidate_props {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "system_prop".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.insert(0, "system_prop".to_owned());
            }
            break;
        }
    }

    ctx.set_fonts(fonts);
}
