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

#[test]
fn search_with_collapsed_nodes_expands_containers() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = jsonviewer::model::SAMPLE_JSON.to_string();
    assert!(m.parse_and_build_tree(true, &s));

    // Collapse all containers
    m.collapse_all();
    assert!(!m.expanded_nodes.contains("$.statistics"));
    assert!(!m.visible_tree_rows.iter().any(|r| r.path == "$.statistics.downloads"));

    // Search for "downloads", which is inside collapsed $.statistics
    m.search_query = "downloads".to_string();
    m.search_start(&s);

    assert_eq!(m.search_results.len(), 1);
    assert_eq!(m.search_results[0], "$.statistics.downloads");
    // $.statistics container must now be expanded!
    assert!(m.expanded_nodes.contains("$.statistics"));
    // "downloads" row must now be present in visible rows!
    assert!(m.visible_tree_rows.iter().any(|r| r.path == "$.statistics.downloads"));
    assert_eq!(m.requested_scroll_path.as_deref(), Some("$.statistics.downloads"));
}

#[test]
fn search_container_node_itself_expands() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = jsonviewer::model::SAMPLE_JSON.to_string();
    assert!(m.parse_and_build_tree(true, &s));

    // Collapse all
    m.collapse_all();
    assert!(!m.expanded_nodes.contains("$.author"));
    assert!(!m.visible_tree_rows.iter().any(|r| r.path == "$.author.name"));

    // Search for "author", which is a container node itself
    m.search_query = "author".to_string();
    m.search_start(&s);

    assert!(!m.search_results.is_empty());
    // $.author itself must now be expanded!
    assert!(m.expanded_nodes.contains("$.author"));
    // Child rows like "name" must now be visible in the tree
    assert!(m.visible_tree_rows.iter().any(|r| r.path == "$.author.name"));
}

#[test]
fn search_when_root_itself_collapsed_expands() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = jsonviewer::model::SAMPLE_JSON.to_string();
    assert!(m.parse_and_build_tree(true, &s));

    // Collapse even the root node ($)
    m.collapse_all();
    m.toggle_expand("$");
    assert!(!m.expanded_nodes.contains("$"));
    assert_eq!(m.visible_tree_rows.len(), 1); // Only root row itself

    // Search for "rating"
    m.search_query = "rating".to_string();
    m.search_start(&s);

    assert!(m.expanded_nodes.contains("$"));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == "$.rating"));
    assert_eq!(m.requested_scroll_path.as_deref(), Some("$.rating"));
}

#[test]
fn scrolling_request_consumed_in_egui_frame() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);

    let s = Settings::default();
    let mut app = ViewerApp::new(s, None);
    app.doc.collapse_all();

    app.search_input = "downloads".to_string();
    app.do_search_go();

    assert_eq!(app.doc.requested_scroll_path.as_deref(), Some("$.statistics.downloads"));

    // Frame 1: tree view renders and consumes the scroll request to center the row
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });

    // The scroll request must have been consumed
    assert!(app.doc.requested_scroll_path.is_none());

    // Frame 2: subsequent frames leave requested_scroll_path as None so user scrolling is never locked
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });

    assert!(app.doc.requested_scroll_path.is_none());
}

#[test]
fn search_next_previous_cycling_expands_and_scrolls() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = jsonviewer::model::SAMPLE_JSON.to_string();
    assert!(m.parse_and_build_tree(true, &s));

    m.collapse_all();
    // Search query "local" matches $.author.email
    // Search query "1" matches multiple places across collapsed containers
    m.search_query = "8".to_string();
    m.search_start(&s);
    assert!(m.search_results.len() >= 2);

    let first_target = m.search_results[0].clone();
    assert_eq!(m.requested_scroll_path.as_deref(), Some(first_target.as_str()));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == first_target));

    // Next match
    m.search_next(&s);
    let second_target = m.search_results[1].clone();
    assert_eq!(m.requested_scroll_path.as_deref(), Some(second_target.as_str()));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == second_target));

    // Previous match returns to first
    m.search_previous(&s);
    assert_eq!(m.requested_scroll_path.as_deref(), Some(first_target.as_str()));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == first_target));
}


