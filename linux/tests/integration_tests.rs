use jsonviewer::app::ViewerApp;
use jsonviewer::desktop::{app_icon, DESKTOP_ENTRY_STR, ICON_PNG_BYTES};
use jsonviewer::json::JSONParser;
use jsonviewer::model::{AppTab, DocumentModel};
use jsonviewer::python::parse_python_literal;
use jsonviewer::settings::Settings;
use jsonviewer::setup_fonts;

/// Wait for the cancellable background search to deliver its results.
fn settle_search(m: &mut DocumentModel) {
    for _ in 0..4000 {
        m.poll_search_results();
        if !m.search_in_flight {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("background search did not finish");
}

fn run_search(m: &mut DocumentModel, s: &Settings, query: &str) {
    m.search_query = query.to_string();
    m.search_start(s, false);
    settle_search(m);
}

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
    run_search(&mut m, &s, "bob");
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
    m.selected_id = Some("$.profile".to_string());
    let (parent, props) = m.properties_for_selected();
    assert_eq!(parent.unwrap().id, "$");
    assert_eq!(props.len(), 2);
    assert_eq!(props[0].name, "name");
    assert_eq!(props[1].name, "age");

    // Navigate to parent
    m.navigate_to_parent();
    assert_eq!(m.selected_id.as_deref(), Some("$"));
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
    settle_search(&mut app.doc);
    app.do_search_next();
    app.do_search_prev();

    // Test leaf expansion
    if let Some(first_leaf) = app.doc.visible_tree_rows.iter().find(|r| !r.is_container).cloned() {
        app.doc.toggle_expand_leaf(&first_leaf.id);
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
    settle_search(&mut app.doc);
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
    run_search(&mut m, &s, "downloads");

    assert_eq!(m.search_results.len(), 1);
    assert_eq!(m.search_results[0], "$.statistics.downloads");
    // $.statistics container must now be expanded!
    assert!(m.expanded_nodes.contains("$.statistics"));
    // "downloads" row must now be present in visible rows!
    assert!(m.visible_tree_rows.iter().any(|r| r.path == "$.statistics.downloads"));
    assert_eq!(m.requested_scroll_id.as_deref(), Some("$.statistics.downloads"));
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
    run_search(&mut m, &s, "author");

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
    run_search(&mut m, &s, "rating");

    assert!(m.expanded_nodes.contains("$"));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == "$.rating"));
    assert_eq!(m.requested_scroll_id.as_deref(), Some("$.rating"));
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
    settle_search(&mut app.doc);

    assert_eq!(app.doc.requested_scroll_id.as_deref(), Some("$.statistics.downloads"));
    let navigation_request = app.doc.tree_navigation_request;

    // Frame 1: tree view renders and consumes the scroll request to center the row
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });

    // The scroll request must have been consumed
    assert!(app.doc.requested_scroll_id.is_none());
    assert_eq!(app.handled_navigation_request, navigation_request);

    // Frame 2: subsequent frames leave requested_scroll_id as None so user scrolling is never locked
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });

    assert!(app.doc.requested_scroll_id.is_none());
    assert!(app.pending_scroll_frames == 0);
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
    run_search(&mut m, &s, "8");
    assert!(m.search_results.len() >= 2);

    let first_target = m.search_results[0].clone();
    assert_eq!(m.requested_scroll_id.as_deref(), Some(first_target.as_str()));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == first_target));

    // Next match
    m.search_next(&s);
    let second_target = m.search_results[1].clone();
    assert_eq!(m.requested_scroll_id.as_deref(), Some(second_target.as_str()));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == second_target));

    // Previous match returns to first
    m.search_previous(&s);
    assert_eq!(m.requested_scroll_id.as_deref(), Some(first_target.as_str()));
    assert!(m.visible_tree_rows.iter().any(|r| r.path == first_target));
}

#[test]
fn mouse_wheel_scroll_direction_inverts_when_not_natural() {
    use eframe::App;

    let s = Settings::default();
    assert!(!s.natural_scrolling, "Natural scrolling must be false by default for traditional mouse scrolling");

    let mut app = ViewerApp::new(s, None);
    let ctx = egui::Context::default();

    // Test with natural_scrolling = false (default)
    let mut raw_input = egui::RawInput::default();
    raw_input.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -5.0),
        modifiers: egui::Modifiers::default(),
    });

    app.raw_input_hook(&ctx, &mut raw_input);

    match &raw_input.events[0] {
        egui::Event::MouseWheel { delta, .. } => {
            assert_eq!(delta.y, 5.0, "Vertical delta should be inverted to produce traditional scroll direction");
        }
        _ => panic!("Expected MouseWheel event"),
    }

    // Test with natural_scrolling = true
    app.settings.natural_scrolling = true;
    let mut raw_input2 = egui::RawInput::default();
    raw_input2.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -5.0),
        modifiers: egui::Modifiers::default(),
    });

    app.raw_input_hook(&ctx, &mut raw_input2);

    match &raw_input2.events[0] {
        egui::Event::MouseWheel { delta, .. } => {
            assert_eq!(delta.y, -5.0, "Vertical delta should not be inverted when natural scrolling is explicitly enabled");
        }
        _ => panic!("Expected MouseWheel event"),
    }

    // Test independent scrolls (axis-locking):
    // Vertical swipe with horizontal drift should zero out horizontal drift
    app.settings.natural_scrolling = false;
    let mut raw_input3 = egui::RawInput::default();
    raw_input3.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(1.5, -8.0),
        modifiers: egui::Modifiers::default(),
    });

    app.raw_input_hook(&ctx, &mut raw_input3);

    match &raw_input3.events[0] {
        egui::Event::MouseWheel { delta, .. } => {
            assert_eq!(delta.x, 0.0, "Horizontal drift must be locked to 0 during vertical scroll");
            assert_eq!(delta.y, 8.0, "Vertical delta should be inverted and active");
        }
        _ => panic!("Expected MouseWheel event"),
    }

    // Horizontal swipe with vertical drift should zero out vertical drift
    let mut raw_input4 = egui::RawInput::default();
    raw_input4.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(10.0, -1.0),
        modifiers: egui::Modifiers::default(),
    });

    app.raw_input_hook(&ctx, &mut raw_input4);

    match &raw_input4.events[0] {
        egui::Event::MouseWheel { delta, .. } => {
            assert_eq!(delta.x, 10.0, "Horizontal delta should be preserved");
            assert_eq!(delta.y, 0.0, "Vertical drift must be locked to 0 during horizontal scroll");
        }
        _ => panic!("Expected MouseWheel event"),
    }

}

#[test]
fn settings_deserializes_legacy_config_without_natural_scrolling() {
    let legacy_json = r#"{
        "indentSpaces": 4,
        "sortKeysAlphabetically": true,
        "escapeSlashesInStringify": false,
        "autoUnwrapStringified": false,
        "defaultTab": "Viewer",
        "fontSize": 15.0,
        "wrapLines": false
    }"#;

    let s: Settings = serde_json::from_str(legacy_json).expect("Must deserialize legacy config");
    assert_eq!(s.indent_spaces, 4);
    assert!(!s.natural_scrolling, "Legacy configs without naturalScrolling field must default to false");
}



fn build_large_doc(items: usize) -> String {
    let mut doc = String::from("{\"needle\":\"early match\",\"items\":[");
    for i in 0..items {
        if i > 0 {
            doc.push(',');
        }
        doc.push_str(&format!("{{\"id\":{},\"group\":{{\"value\":{}}}}}", i, i));
    }
    doc.push_str("]}");
    doc
}

#[test]
fn repeated_object_keys_get_unique_tree_ids() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = r#"{"count":3,"count":23423,"count":23423}"#.to_string();
    assert!(m.parse_and_build_tree(true, &s));

    let root = m.root.as_ref().unwrap();
    assert_eq!(root.children.len(), 3, "Parser keeps every repeated key");
    let ids: std::collections::HashSet<&str> = root.children.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 3, "Repeated object keys need unique tree node IDs");
    let values: Vec<String> = root.children.iter().map(|c| c.value_string()).collect();
    assert_eq!(values, vec!["3".to_string(), "23423".to_string(), "23423".to_string()]);

    m.expand_all();
    let row_ids: Vec<String> = m
        .visible_tree_rows
        .iter()
        .filter(|r| r.key == "count")
        .map(|r| r.id.clone())
        .collect();
    assert_eq!(row_ids.len(), 3, "Every repeated entry must appear in the tree");
    let unique: std::collections::HashSet<&String> = row_ids.iter().collect();
    assert_eq!(unique.len(), 3);
    // Rows still share the JSON path shown to the user.
    for row in m.visible_tree_rows.iter().filter(|r| r.key == "count") {
        assert_eq!(row.path, "$.count");
    }

    // Repeated entries can be selected independently.
    m.selected_id = Some(row_ids[1].clone());
    let (_, props) = m.properties_for_selected();
    assert_eq!(props.len(), 3);
    assert_eq!(props[1].value, "23423");
    assert_eq!(props[1].path, "$.count");
    let prop_ids: std::collections::HashSet<String> = props.into_iter().map(|p| p.id).collect();
    assert_eq!(prop_ids.len(), 3, "Property rows keep distinct IDs for jumping");

    // Expanding one repeated container must not expand its sibling.
    m.raw_text = r#"{"a":{"v":1},"a":{"v":2}}"#.to_string();
    assert!(m.parse_and_build_tree(true, &s));
    m.toggle_expand("$.a");
    assert_eq!(m.visible_tree_rows.len(), 4, "Only the selected duplicate shows its child");
    m.toggle_expand("$.a#2");
    assert_eq!(m.visible_tree_rows.len(), 5);
}

#[test]
fn split_editing_keeps_last_valid_tree_and_preserves_navigation() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = r#"{"user":{"name":"ann"},"keep":{"deep":{"value":1}}}"#.to_string();
    assert!(m.parse_and_build_tree(true, &s));

    m.expand_subtree("$.keep.deep");
    m.select_and_reveal("$.keep.deep.value");
    assert_eq!(m.selected_id.as_deref(), Some("$.keep.deep.value"));
    let rows_before = m.visible_tree_rows.len();

    // Editor text becomes invalid: the last parsed tree stays and is flagged out of date.
    m.raw_text = "{ \"user\": ".to_string();
    m.mark_edited();
    assert!(!m.parse_and_build_tree(true, &s));
    assert!(m.is_dirty, "Tree stays out of date while the text is invalid");
    assert!(m.parse_error.is_some());
    assert!(m.root.is_some(), "Last successfully parsed tree is kept");
    assert_eq!(m.visible_tree_rows.len(), rows_before);
    assert_eq!(m.selected_id.as_deref(), Some("$.keep.deep.value"));

    // A successful parse keeps the selection and the expanded branches.
    m.raw_text = r#"{"user":{"name":"bob"},"keep":{"deep":{"value":2},"extra":3}}"#.to_string();
    assert!(m.parse_and_build_tree(true, &s));
    assert!(!m.is_dirty);
    assert!(m.parse_error.is_none());
    assert_eq!(m.selected_id.as_deref(), Some("$.keep.deep.value"));
    assert!(m.expanded_nodes.contains("$.keep"));
    assert!(m.expanded_nodes.contains("$.keep.deep"));

    // Missing branches collapse and a vanished selection falls back to the root.
    m.raw_text = r#"{"other":1}"#.to_string();
    assert!(m.parse_and_build_tree(true, &s));
    assert_eq!(m.selected_id.as_deref(), Some("$"));
    assert!(!m.expanded_nodes.contains("$.keep"));
    assert!(m.expanded_nodes.contains("$"));
}

#[test]
fn tree_navigation_expands_ancestors_before_requesting_scroll() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = jsonviewer::model::SAMPLE_JSON.to_string();
    assert!(m.parse_and_build_tree(true, &s));

    m.collapse_all();
    m.toggle_expand("$"); // collapse the root as well
    assert_eq!(m.visible_tree_rows.len(), 1);

    let before = m.tree_navigation_request;
    run_search(&mut m, &s, "downloads");

    assert!(m.tree_navigation_request > before, "Navigation request is signalled after the rebuild");
    let target = m.requested_scroll_id.clone().expect("scroll requested");
    assert!(
        m.visible_tree_rows.iter().any(|r| r.id == target),
        "Ancestors are expanded before the scroll is requested"
    );
}

#[test]
fn scroll_request_survives_until_the_row_exists() {
    let ctx = egui::Context::default();
    let s = Settings::default();
    let mut app = ViewerApp::new(s, None);

    app.doc.tree_navigation_request += 1;
    app.doc.requested_scroll_id = Some("$.not.a.real.node".to_string());

    // Frame 1: the row is missing, so the request must stay pending.
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });
    assert_eq!(app.doc.requested_scroll_id.as_deref(), Some("$.not.a.real.node"));
    assert!(app.pending_scroll_frames > 0);

    // Give up after a bounded number of frames so user scrolling is never locked.
    for _ in 0..4 {
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.show_tree_view(ctx, ui);
            });
        });
    }
    assert!(app.doc.requested_scroll_id.is_none());
    assert_eq!(app.pending_scroll_frames, 0);
}

#[test]
fn search_from_bottom_of_large_tree_scrolls_to_early_match() {
    let s = Settings::default();
    let mut app = ViewerApp::new(s.clone(), None);
    app.doc.raw_text = build_large_doc(400);
    assert!(app.doc.parse_and_build_tree(true, &s));
    app.doc.expand_all();
    assert!(app.doc.visible_tree_rows.len() > 800, "Tree is far taller than the viewport");

    app.search_input = "needle".to_string();
    app.do_search_go();
    settle_search(&mut app.doc);

    let target = app.doc.requested_scroll_id.clone().expect("scroll requested");
    let idx = app
        .doc
        .visible_tree_rows
        .iter()
        .position(|r| r.id == target)
        .expect("match row is part of the rebuilt tree");
    assert!(idx < 3, "Match near the beginning of the document stays near the top");

    let ctx = egui::Context::default();
    let request = app.doc.tree_navigation_request;
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });
    assert!(app.doc.requested_scroll_id.is_none(), "Scroll request is consumed");
    assert_eq!(app.handled_navigation_request, request);
}

#[test]
fn search_runs_in_background_and_reports_progress() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = build_large_doc(2000);
    assert!(m.parse_and_build_tree(true, &s));

    m.search_query = "value".to_string();
    m.search_start(&s, false);
    // The query runs off the UI thread: results are not applied synchronously.
    assert!(m.search_in_flight || m.search_status == "Searching…".to_string());
    assert!(m.search_results.is_empty(), "Old matches must not linger while searching");

    settle_search(&mut m);
    assert!(!m.search_in_flight);
    assert!(!m.search_results.is_empty());
    assert!(m.search_status.contains("of "), "Match count is shown: {}", m.search_status);
    assert!(m.selected_id.is_some(), "First match is revealed");
}

#[test]
fn editing_the_query_discards_in_flight_results() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = build_large_doc(2000);
    assert!(m.parse_and_build_tree(true, &s));

    m.search_query = "value".to_string();
    m.search_start(&s, false);
    // Clearing the search cancels the worker and invalidates its result.
    m.clear_search();
    assert!(!m.search_in_flight);
    assert!(m.search_results.is_empty());
    assert!(m.search_status.is_empty());

    for _ in 0..20 {
        m.poll_search_results();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(m.search_results.is_empty(), "Canceled query must not deliver matches");
    assert!(m.selected_id.is_none() || m.search_results.is_empty());

    // A newer query still works afterwards.
    run_search(&mut m, &s, "group");
    assert!(!m.search_results.is_empty());
}

#[test]
fn search_previous_while_running_lands_on_last_match() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = build_large_doc(500);
    assert!(m.parse_and_build_tree(true, &s));

    m.search_query = "value".to_string();
    m.search_start(&s, false);
    m.search_previous(&s); // pressed before results landed
    settle_search(&mut m);

    assert_eq!(m.selected_id.as_deref(), m.search_results.last().map(|s| s.as_str()));
    assert!(m.search_status.starts_with(&format!("{} of ", m.search_results.len())));
}

#[test]
fn clearing_search_requests_tree_scroll_to_root() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = jsonviewer::model::SAMPLE_JSON.to_string();
    assert!(m.parse_and_build_tree(true, &s));
    run_search(&mut m, &s, "downloads");
    assert!(!m.search_results.is_empty());

    let before = m.tree_scroll_to_top_request;
    m.clear_search();
    assert!(m.tree_scroll_to_top_request > before, "Clearing search asks the tree to return to the root");
    assert!(m.search_results.is_empty());
    assert!(m.requested_scroll_id.is_none());

    // The view applies the request once.
    let ctx = egui::Context::default();
    let mut app = ViewerApp::new(s, None);
    app.doc = m;
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_tree_view(ctx, ui);
        });
    });
    assert_eq!(app.handled_scroll_to_top_request, app.doc.tree_scroll_to_top_request);
}

#[test]
fn returning_to_viewer_restores_scroll_for_selected_match() {
    let s = Settings::default();
    let mut app = ViewerApp::new(s.clone(), None);
    app.doc.raw_text = build_large_doc(400);
    assert!(app.doc.parse_and_build_tree(true, &s));
    app.doc.expand_all();

    app.search_input = "needle".to_string();
    app.do_search_go();
    settle_search(&mut app.doc);
    let selected = app.doc.selected_id.clone().expect("match selected");
    app.doc.requested_scroll_id = None;

    // Switching to the Text tab and back must re-request the scroll even though
    // the selection never changed.
    let mut app = app;
    app.doc.active_tab = AppTab::Text;
    app.last_tab = AppTab::Text;
    app.doc.active_tab = AppTab::Viewer;
    // Mirror the tab synchronization the frame loop performs.
    let settings = app.settings.clone();
    app.doc.parse_and_build_tree(true, &settings);
    if !app.doc.search_results.is_empty() {
        app.doc.request_scroll_to_selection();
    }
    assert_eq!(app.doc.selected_id.as_deref(), Some(selected.as_str()));
    assert_eq!(app.doc.requested_scroll_id.as_deref(), Some(selected.as_str()));
    assert!(app.doc.visible_tree_rows.iter().any(|r| r.id == selected));
}
