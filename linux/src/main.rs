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

fn main() -> eframe::Result<()> {
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
        Box::new(move |_cc| Ok(Box::new(ViewerApp::new(settings, initial)))),
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
}
