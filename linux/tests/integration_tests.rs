use jsonviewer::app::ViewerApp;
use jsonviewer::desktop::{app_icon, DESKTOP_ENTRY_STR, ICON_PNG_BYTES};
use jsonviewer::json::JSONParser;
use jsonviewer::model::{AppTab, DocumentModel};
use jsonviewer::python::parse_python_literal;
use jsonviewer::settings::Settings;
use jsonviewer::setup_fonts;

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
    assert_eq!(m.visible_tree_rows.len(), 3);

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
    assert!(!ICON_PNG_BYTES.is_empty());
    assert!(DESKTOP_ENTRY_STR.contains("Icon=jsonviewer"));
    assert!(DESKTOP_ENTRY_STR.contains("StartupWMClass=jsonviewer"));
    assert!(app_icon().is_some());
}

#[test]
fn font_setup_executes_without_panic() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
}

#[test]
fn egui_frame_simulations_all_tabs() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);

    let s = Settings::default();
    let mut app = ViewerApp::new(s, None);

    // Frame 1: Viewer Tab
    app.doc.active_tab = AppTab::Viewer;
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
    app.doc.active_tab = AppTab::Text;
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_main_header(ui);
            app.show_text_toolbar(ctx, ui);
        });
    });

    // Frame 3: Split Tab
    app.doc.active_tab = AppTab::Split;
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

#[test]
fn large_5mb_file_parsing_search_and_virtualization() {
    let path = std::path::Path::new("/tmp/5MB.json");
    if !path.is_file() {
        return; // Skip if test file wasn't downloaded
    }
    let s = Settings::default();
    let mut app = ViewerApp::new(s.clone(), Some(path.to_string_lossy().to_string()));
    assert!(app.doc.root.is_some());
    assert!(app.doc.visible_tree_rows.len() > 1000);

    // Test search on large document
    app.search_input = "anes".to_string();
    app.do_search_go();
    assert!(!app.doc.search_results.is_empty());
    app.do_search_next();
    app.do_search_prev();

    // Test egui frame virtualization does not crash or hang
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });
}
