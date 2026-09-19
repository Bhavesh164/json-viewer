mod app;
mod clipboard;
mod desktop;
mod json;
mod model;
mod python;
mod settings;

use app::ViewerApp;
use settings::SettingsStore;

const HELP: &str = "\
jsonviewer - JSON Viewer and Formatter (Linux)

Usage:
  jsonviewer [FILE]        open FILE (.json) or raw JSON text
  jsonviewer --install     install .desktop launcher and icons to ~/.local/share
  jsonviewer --help        print this help

Config: $XDG_CONFIG_HOME/JSONViewer/config.json (~/.config/JSONViewer/config.json)
";

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let msg = format!(
            "================ JSON VIEWER CRASH REPORT ================\n\
             Timestamp: {:?}\n\
             Panic Info: {}\n\
             Backtrace:\n{}\n\
             ========================================================\n",
            std::time::SystemTime::now(),
            info,
            backtrace
        );
        eprintln!("{}", msg);
        let _ = std::fs::write("/tmp/jsonviewer_crash.log", &msg);
        if let Ok(home) = std::env::var("HOME") {
            let log_dir = std::path::PathBuf::from(home).join(".local/state/jsonviewer");
            let _ = std::fs::create_dir_all(&log_dir);
            let _ = std::fs::write(log_dir.join("crash.log"), &msg);
        }
    }));
}

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

fn main() -> eframe::Result<()> {
    install_panic_hook();

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", HELP);
        return Ok(());
    }
    if args.iter().any(|a| a == "--install") {
        desktop::ensure_desktop_integration();
        println!("JSON Viewer desktop entry and icons installed successfully.");
        return Ok(());
    }

    // Ensure desktop entry and icons are registered for Omarchy / XDG launchers
    desktop::ensure_desktop_integration();

    let initial = args.first().cloned();

    let store = SettingsStore::new();
    let settings = store.settings.clone();
    // Persist clamped defaults on first run (mirrors mouseless/linux pattern).
    store.save();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([720.0, 480.0])
        .with_app_id("jsonviewer");

    if let Some(icon) = desktop::app_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "JSON Viewer",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(ViewerApp::new(settings, initial)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use crate::json::JSONParser;
    use crate::model::DocumentModel;
    use crate::python::parse_python_literal;
    use crate::settings::Settings;

    #[test]
    fn parses_sample_and_formats() {
        let v = JSONParser::parse(r#"{"b":2,"a":1}"#).unwrap();
        assert_eq!(v.format(2, false, false), "{\n  \"b\": 2,\n  \"a\": 1\n}");
        assert_eq!(v.format(2, true, false), "{\n  \"a\": 1,\n  \"b\": 2\n}");
        assert_eq!(v.minify(false), r#"{"b":2,"a":1}"#);
    }

    #[test]
    fn python_literal_converts() {
        let v = parse_python_literal("{'a': True, 'b': None}").unwrap();
        assert_eq!(v.format(2, false, false), "{\n  \"a\": true,\n  \"b\": null\n}");
    }

    #[test]
    fn document_search_and_transforms() {
        let s = Settings::default();
        let mut m = DocumentModel::new(&s);
        m.raw_text = r#"{"users":[{"name":"ann"},{"name":"bob"}]}"#.to_string();
        assert!(m.parse_and_build_tree(true, &s));
        m.search_query = "bob".to_string();
        m.search_start(&s);
        assert_eq!(m.search_results.len(), 1);
        assert!(m.beautify(&s).is_ok());
        assert!(m.raw_text.contains('\n'));
    }

    #[test]
    fn invalid_json_reports_line_col() {
        let err = JSONParser::parse("{\n\"a\": }").unwrap_err();
        assert!(err.line >= 2);
    }

    #[test]
    fn visible_tree_rows_flattening_and_expansion() {
        let s = Settings::default();
        let mut m = DocumentModel::new(&s);
        m.raw_text = r#"{"a": 1, "nested": {"b": 2, "c": 3}}"#.to_string();
        assert!(m.parse_and_build_tree(true, &s));

        // Initially only root is expanded: root row + immediate children ("a", "nested")
        assert_eq!(m.visible_tree_rows.len(), 3);
        assert_eq!(m.visible_tree_rows[0].key, "JSON");
        assert_eq!(m.visible_tree_rows[1].key, "a");
        assert_eq!(m.visible_tree_rows[2].key, "nested");

        // Expand "nested"
        m.toggle_expand("$.nested");
        assert_eq!(m.visible_tree_rows.len(), 5);
        assert_eq!(m.visible_tree_rows[3].key, "b");
        assert_eq!(m.visible_tree_rows[4].key, "c");

        // Collapse all
        m.collapse_all();
        assert_eq!(m.visible_tree_rows.len(), 3); // Root open, children closed

        // Expand all
        m.expand_all();
        assert_eq!(m.visible_tree_rows.len(), 5);
    }

    #[test]
    fn property_grid_and_parent_drilldown() {
        let s = Settings::default();
        let mut m = DocumentModel::new(&s);
        m.raw_text = r#"{"profile": {"name": "Alice", "age": 30}}"#.to_string();
        assert!(m.parse_and_build_tree(true, &s));

        // Select nested object
        m.selected_path = Some("$.profile".to_string());
        let (parent, props) = m.properties_for_selected();
        assert_eq!(parent.unwrap().0, "$");
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].name, "name");
        assert_eq!(props[1].name, "age");

        // Navigate to parent
        m.navigate_to_parent();
        assert_eq!(m.selected_path.as_deref(), Some("$"));
    }

    #[test]
    fn desktop_embedded_assets_valid() {
        use crate::desktop::{app_icon, DESKTOP_ENTRY_STR, ICON_PNG_BYTES};
        assert!(!ICON_PNG_BYTES.is_empty());
        assert!(DESKTOP_ENTRY_STR.contains("Icon=jsonviewer"));
        assert!(DESKTOP_ENTRY_STR.contains("StartupWMClass=jsonviewer"));
        assert!(app_icon().is_some());
    }

    #[test]
    fn font_setup_executes_without_panic() {
        let ctx = egui::Context::default();
        super::setup_fonts(&ctx);
    }

    #[test]
    fn egui_frame_simulations_all_tabs() {
        let ctx = egui::Context::default();
        super::setup_fonts(&ctx);

        let s = Settings::default();
        let mut app = ViewerApp::new(s, None);

        // Frame 1: Viewer Tab
        app.doc.active_tab = crate::model::AppTab::Viewer;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_main_header(ui);
                app.show_tree_toolbar(ctx, ui);
                app.show_tree_view(ctx, ui);
                app.show_property_grid(ctx, ui);
                app.show_search_toolbar(ui);
            });
            app.show_settings_window(ctx);
            app.show_shortcuts_window(ctx);
            app.show_about_window(ctx);
        });

        // Frame 2: Text Tab
        app.doc.active_tab = crate::model::AppTab::Text;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_main_header(ui);
                app.show_text_toolbar(ctx, ui);
            });
        });

        // Frame 3: Split Tab
        app.doc.active_tab = crate::model::AppTab::Split;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_main_header(ui);
                ui.columns(2, |cols| {
                    app.show_text_toolbar(ctx, &mut cols[0]);
                    app.show_tree_toolbar(ctx, &mut cols[1]);
                    app.show_tree_view(ctx, &mut cols[1]);
                });
            });
        });

        // Test transformations
        app.apply_transform("format");
        app.apply_transform("minify");
        app.apply_transform("stringify");
        app.apply_transform("unescape");
        app.apply_transform("j2p");
        app.apply_transform("p2j");

        // Test search
        app.search_input = "macOS".to_string();
        app.do_search_go();
        app.do_search_next();
        app.do_search_prev();

        // Test leaf expansion
        if let Some(first_leaf) = app.doc.visible_tree_rows.iter().find(|r| !r.is_container).cloned() {
            app.doc.toggle_expand_leaf(&first_leaf.path);
        }

        // Run frame with expanded leaf
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_tree_view(ctx, ui);
            });
        });
    }
}
