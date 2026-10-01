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
            app.show_text_toolbar(&ctx, ui);
        });
    });

    // Frame 3: Split Tab
    app.doc.active_tab = AppTab::Split;
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.show_main_header(ui);
            ui.columns(2, |cols| {
                app.show_text_toolbar(&ctx, &mut cols[0]);
                app.show_tree_toolbar(ctx, &mut cols[1]);
                app.show_tree_view(&ctx, &mut cols[1]);
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

// ---------------------------------------------------------------- code editor

use jsonviewer::code_editor::{CodeEditor, Cursor, TextBuffer};

fn editor_raw() -> egui::RawInput {
    let mut raw = egui::RawInput::default();
    raw.screen_rect = Some(egui::Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(1200.0, 800.0),
    ));
    raw.viewport_id = egui::ViewportId::ROOT;
    raw
}

fn mods(m: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: m,
    }
}

fn key(k: egui::Key) -> egui::Event {
    egui::Event::Key {
        key: k,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::default(),
    }
}

/// Run one editor frame, optionally returning whether the text changed.
fn frame(ctx: &egui::Context, editor: &mut CodeEditor, events: Vec<egui::Event>) -> bool {
    let mut raw = editor_raw();
    raw.events = events;
    let mut changed = false;
    let _ = ctx.run(raw, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            changed = editor.show(ui, egui::FontId::monospace(13.0));
        });
    });
    changed
}

/// Give the editor keyboard focus the same way the Text tab does.
fn focus(ctx: &egui::Context, editor: &mut CodeEditor) {
    editor.request_focus();
    let _ = ctx.run(editor_raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, egui::FontId::monospace(13.0));
        });
    });
}

#[test]
fn line_index_matches_document() {
    let mut buf = TextBuffer::new("alpha\nbeta\n\ngamma\n");
    assert_eq!(buf.line_count(), 5, "a trailing newline opens a final empty line");
    assert_eq!(buf.line_str(0), "alpha");
    assert_eq!(buf.line_str(2), "");
    assert_eq!(buf.line_str(4), "");
    assert_eq!(buf.line_cols(0), 5);
    assert_eq!(buf.max_cols(), 5);

    // Byte offsets and columns round-trip.
    for (line, col) in [(0, 0), (0, 5), (1, 2), (2, 0), (3, 4), (4, 0)] {
        let c = Cursor { line, col };
        assert_eq!(buf.cursor_of(buf.byte_of(c)), c, "cursor round-trip {c:?}");
    }

    // Splicing inside a line keeps the index and width statistics correct.
    let (removed, end) = buf.replace(Cursor { line: 0, col: 1 }..Cursor { line: 0, col: 3 }, "XYZW");
    assert_eq!(removed, "lp");
    assert_eq!(end, Cursor { line: 0, col: 5 }, "caret lands after the inserted text");
    assert_eq!(buf.text(), "aXYZWha\nbeta\n\ngamma\n");
    assert_eq!(buf.line_count(), 5);
    assert_eq!(buf.line_cols(0), 7, "the edited line is now the widest");
    assert_eq!(buf.max_cols(), 7);
    assert_eq!(buf.cursor_of(buf.byte_of(Cursor { line: 3, col: 2 })), Cursor { line: 3, col: 2 });

    // Adding a newline reindexes, and the widest-line statistic is recomputed.
    let mut buf = TextBuffer::new("aaaaaaaaaaaaaaaaaaaa\nb\n");
    assert_eq!(buf.max_cols(), 20);
    let _ = buf.replace(Cursor { line: 0, col: 0 }..Cursor { line: 0, col: 0 }, "\n");
    assert_eq!(buf.line_count(), 4);
    assert_eq!(buf.line_str(1), "aaaaaaaaaaaaaaaaaaaa");
    assert_eq!(buf.max_cols(), 20, "the longest line moved down one line");
}

#[test]
fn line_index_handles_utf8_and_tabs() {
    let buf = TextBuffer::new("a\tb\nnaïve\n日本語\n");
    assert!(!buf.is_simple(), "tabs and non-ASCII disable the byte==column fast path");
    // A tab advances to the next 4-column stop. Accented Latin letters stay
    // narrow in a monospaced grid; CJK is double width.
    assert_eq!(buf.line_cols(0), 5, "'a' then tab to column 4, then 'b'");
    assert_eq!(buf.line_cols(1), 5, "n a ï v e, all narrow");
    assert_eq!(buf.line_cols(2), 6, "three double-width characters");
    assert_eq!(buf.max_cols(), 6);
    for (line, col) in [(0, 0), (0, 5), (1, 5), (2, 6)] {
        let c = Cursor { line, col };
        assert_eq!(buf.cursor_of(buf.byte_of(c)), c, "round-trip {c:?}");
    }
    // A column past the end of the line clamps instead of running off it.
    let past = Cursor { line: 1, col: 99 };
    assert_eq!(buf.clamp(past), Cursor { line: 1, col: 5 });
}

#[test]
fn typing_reports_change_and_moves_the_caret() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut editor = CodeEditor::new("ab");
    focus(&ctx, &mut editor);

    let changed = frame(
        &ctx,
        &mut editor,
        vec![egui::Event::Text("x".to_string())],
    );
    assert!(changed, "typing reports a change so the document is re-parsed");
    assert_eq!(editor.text(), "xab");
    assert_eq!(editor.cursor(), Cursor { line: 0, col: 1 });
}

#[test]
fn editing_navigation_and_undo_redo() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut editor = CodeEditor::new("one\ntwo\nthree\n");
    focus(&ctx, &mut editor);

    // Walk down two lines, then extend the selection to the end of that line.
    frame(&ctx, &mut editor, vec![key(egui::Key::ArrowDown)]);
    frame(&ctx, &mut editor, vec![key(egui::Key::ArrowDown)]);
    assert_eq!(editor.cursor(), Cursor { line: 2, col: 0 });

    let mut shifted = key(egui::Key::End);
    if let egui::Event::Key { modifiers, .. } = &mut shifted {
        modifiers.shift = true;
    }
    frame(&ctx, &mut editor, vec![shifted]);
    assert_eq!(editor.selected_text(), "three", "shift+End selects to end of line");
    assert!(editor.has_selection());

    // Typing replaces the selection; undo and redo walk the same steps. The
    // trailing newline survives because only "three" was selected.
    frame(&ctx, &mut editor, vec![egui::Event::Text("X".to_string())]);
    assert_eq!(editor.text(), "one\ntwo\nX\n");
    editor.undo();
    assert_eq!(editor.text(), "one\ntwo\nthree\n");
    editor.redo();
    assert_eq!(editor.text(), "one\ntwo\nX\n");

    // Backspace with the caret after "X" removes the character.
    frame(&ctx, &mut editor, vec![key(egui::Key::Backspace)]);
    assert_eq!(editor.text(), "one\ntwo\n\n");
    editor.undo();
    assert_eq!(editor.text(), "one\ntwo\nX\n");

    // Backspace at the start of a line joins it to the previous line.
    frame(&ctx, &mut editor, vec![key(egui::Key::Home)]);
    assert_eq!(editor.cursor(), Cursor { line: 2, col: 0 });
    frame(&ctx, &mut editor, vec![key(egui::Key::Backspace)]);
    assert_eq!(editor.text(), "one\ntwoX\n");
    assert_eq!(editor.cursor(), Cursor { line: 1, col: 3 });
    editor.undo();
    assert_eq!(editor.text(), "one\ntwo\nX\n");
}

#[test]
fn ctrl_select_all_then_delete_clears_the_document() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut editor = CodeEditor::new("alpha\nbeta\n");
    focus(&ctx, &mut editor);

    let mut ctrl_a = key(egui::Key::A);
    if let egui::Event::Key { modifiers, .. } = &mut ctrl_a {
        modifiers.ctrl = true;
    }
    frame(&ctx, &mut editor, vec![ctrl_a]);
    assert_eq!(editor.selected_text(), "alpha\nbeta\n");

    frame(&ctx, &mut editor, vec![key(egui::Key::Backspace)]);
    assert_eq!(editor.text(), "");
    assert_eq!(
        editor.buffer().line_count(),
        1,
        "an empty document is still one empty line"
    );
}

#[test]
fn virtualized_editor_keeps_frames_cheap_on_huge_documents() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);

    // ~8 MB across 100k lines. Frame cost must not scale with the document,
    // which is the whole point of only shaping the visible rows.
    let mut text = String::from("{\n  \"records\": [\n");
    for i in 0..100_000 {
        if i > 0 {
            text.push_str(",\n");
        }
        text.push_str(&format!(
            "    {{\"id\": {i}, \"name\": \"name_{i}\", \"tags\": [\"a\", \"b\"], \
             \"address\": {{\"city\": \"City{}\", \"zip\": \"00000\"}}}}",
            i % 400
        ));
    }
    text.push_str("\n  ]\n}");
    assert!(text.len() > 8_000_000, "fixture should be multi-megabyte");

    let mut big = CodeEditor::new(&text);
    focus(&ctx, &mut big);

    let start = std::time::Instant::now();
    for _ in 0..10 {
        frame(&ctx, &mut big, Vec::new());
    }
    let per_frame = start.elapsed().as_secs_f64() * 100.0;
    assert_eq!(big.text(), text, "painting must not alter the document");
    assert!(
        per_frame < 16.0,
        "8 MB document must stay inside a 16 ms frame, took {per_frame:.2} ms"
    );
}

#[test]
fn minified_single_line_document_stays_cheap() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    // A minified document is one enormous line, so the editor must clip
    // columns to the viewport instead of shaping the whole line every frame.
    let text = format!("[{}{}]", "\"x\",".repeat(400_000), "0");
    assert_eq!(text.lines().count(), 1);

    let mut editor = CodeEditor::new(&text);
    focus(&ctx, &mut editor);

    let start = std::time::Instant::now();
    for _ in 0..10 {
        frame(&ctx, &mut editor, Vec::new());
    }
    let per_frame = start.elapsed().as_secs_f64() * 100.0;
    assert!(
        per_frame < 16.0,
        "single-line document must stay inside a 16 ms frame, took {per_frame:.2} ms"
    );
}

#[test]
fn text_tab_and_split_tab_render_heavy_documents() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let s = Settings::default();

    let mut heavy = String::from("{\n  \"records\": [\n");
    for i in 0..20_000 {
        if i > 0 {
            heavy.push_str(",\n");
        }
        heavy.push_str(&format!("    {{\"id\": {i}, \"name\": \"name_{i}\"}}}}"));
    }
    heavy.push_str("\n  ]\n}");

    let mut app = ViewerApp::new(s, None);
    app.doc.raw_text = heavy;
    app.doc.update_metrics();

    for tab in [AppTab::Text, AppTab::Split] {
        app.doc.active_tab = tab;
        app.last_tab = tab;
        app.editor.set_text(&app.doc.raw_text);
        app.split_editor.set_text(&app.doc.raw_text);

        let mut draw = |app: &mut ViewerApp, ui: &mut egui::Ui| match tab {
            AppTab::Text => {
                app.show_text_toolbar(&ctx, ui);
                ui.separator();
                app.editor.show(ui, egui::FontId::monospace(13.0));
            }
            _ => {
                ui.columns(2, |cols| {
                    app.show_text_toolbar(&ctx, &mut cols[0]);
                    app.split_editor.show(&mut cols[0], egui::FontId::monospace(13.0));
                    app.show_tree_view(&ctx, &mut cols[1]);
                });
            }
        };

        // First frame rasterises the font atlas, so measure the steady state.
        let _ = ctx.run(editor_raw(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
        });
        let start = std::time::Instant::now();
        for _ in 0..5 {
            let _ = ctx.run(editor_raw(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
            });
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0 / 5.0;
        assert!(ms < 16.0, "{tab:?} tab frame on a ~1 MB document: {ms:.2} ms");
    }
}

/// A real click: press and release the primary button at `pos`, then keep the
/// pointer still so egui reports a click rather than a drag.
fn click(ctx: &egui::Context, editor: &mut CodeEditor, pos: egui::Pos2) {
    let mut press = editor_raw();
    press.events.push(egui::Event::PointerMoved(pos));
    press.events.push(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::default(),
    });
    let _ = ctx.run(press, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, egui::FontId::monospace(13.0));
        });
    });

    let mut release = editor_raw();
    release.events.push(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::default(),
    });
    let _ = ctx.run(release, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, egui::FontId::monospace(13.0));
        });
    });
}

#[test]
fn clicking_the_editor_takes_focus_and_allows_typing() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut editor = CodeEditor::new("alpha\nbeta\ngamma\n");

    // Without a click or an explicit focus request there is nothing focused, so
    // the editor grabs the keyboard on its own.
    let _ = ctx.run(editor_raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, egui::FontId::monospace(13.0));
        });
    });
    frame(&ctx, &mut editor, vec![egui::Event::Text("Z".to_string())]);
    assert_eq!(editor.text(), "Zalpha\nbeta\ngamma\n");

    // Click on the third row, which must move the caret and keep focus.
    click(&ctx, &mut editor, egui::pos2(40.0, 3.0 * 20.0 + 10.0));
    assert_eq!(editor.cursor(), Cursor { line: 3, col: 0 });

    frame(&ctx, &mut editor, vec![egui::Event::Text("hi".to_string())]);
    assert_eq!(editor.text(), "Zalpha\nbeta\ngamma\nhi");
}

#[test]
fn clicking_the_editor_steals_focus_from_another_widget() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut editor = CodeEditor::new("alpha\nbeta\n");
    let mut other = String::from("other");
    let mut other_has_focus = true;

    // Draw the editor plus a `TextEdit`. The `TextEdit` asks for focus only
    // while `other_has_focus` is set, exactly like the app's Find field, so
    // only an explicit click on the editor can move focus across.
    fn draw(
        editor: &mut CodeEditor,
        other: &mut String,
        other_has_focus: bool,
        ui: &mut egui::Ui,
    ) {
        editor.show(ui, egui::FontId::monospace(13.0));
        let resp = ui.add(egui::TextEdit::singleline(other));
        if other_has_focus {
            resp.request_focus();
        }
    }

    let run = |ctx: &egui::Context,
               editor: &mut CodeEditor,
               other: &mut String,
               other_has_focus: bool,
               events: Vec<egui::Event>| {
        let mut raw = editor_raw();
        raw.events = events;
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                draw(editor, other, other_has_focus, ui)
            });
        });
    };

    // Let the `TextEdit` take the keyboard.
    run(&ctx, &mut editor, &mut other, other_has_focus, Vec::new());
    run(&ctx, &mut editor, &mut other, other_has_focus, Vec::new());
    other_has_focus = false;

    // Typing now must go to the other field, not into the document.
    run(
        &ctx,
        &mut editor,
        &mut other,
        other_has_focus,
        vec![egui::Event::Text("x".to_string())],
    );
    assert_eq!(other, "otherx", "the focused TextEdit keeps the keyboard");
    assert_eq!(editor.text(), "alpha\nbeta\n", "editor does not steal focus");

    // Clicking inside the editor must hand the keyboard over to it. Click near
    // the start of the first row so the caret lands at 0:0.
    let click_at = egui::pos2(0.0, 5.0);
    for pressed in [true, false] {
        let mut raw = editor_raw();
        raw.events.push(egui::Event::PointerMoved(click_at));
        raw.events.push(egui::Event::PointerButton {
            pos: click_at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        });
        run(&ctx, &mut editor, &mut other, other_has_focus, raw.events);
    }
    run(
        &ctx,
        &mut editor,
        &mut other,
        other_has_focus,
        vec![egui::Event::Text("y".to_string())],
    );
    assert_eq!(editor.text(), "yalpha\nbeta\n", "a click on the editor focuses it");
    assert_eq!(other, "otherx", "the other field no longer receives text");
}

#[test]
fn search_box_keeps_focus_when_the_editor_is_also_shown() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut app = ViewerApp::new(Settings::default(), None);
    app.doc.raw_text = "alpha\nbeta\n".to_string();
    app.editor.set_text(&app.doc.raw_text);

    // One frame to hand focus to the Find field. The editor is on screen in the
    // same pass and runs first, so it must not take focus back.
    app.focus_search = true;
    let _ = ctx.run(editor_raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.editor.show(ui, egui::FontId::monospace(13.0));
            app.show_search_toolbar(ui);
        });
    });

    let mut raw = editor_raw();
    raw.events.push(egui::Event::Text("needle".to_string()));
    let _ = ctx.run(raw, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            app.editor.show(ui, egui::FontId::monospace(13.0));
            app.show_search_toolbar(ui);
        });
    });
    assert_eq!(app.search_input, "needle", "the Find field receives the typing");
    assert_eq!(
        app.editor.text(),
        "alpha\nbeta\n",
        "the editor must not swallow text meant for the Find field"
    );
}

#[test]
fn typing_in_the_text_tab_updates_the_document() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut app = ViewerApp::new(Settings::default(), None);
    app.doc.raw_text = "{\n  \"a\": 1\n}\n".to_string();
    app.doc.update_metrics();
    app.doc.active_tab = AppTab::Text;
    app.last_tab = AppTab::Text;
    app.editor.set_text(&app.doc.raw_text);

    let mut draw = |app: &mut ViewerApp, ui: &mut egui::Ui| {
        app.show_text_tab(&ctx, ui);
    };

    // Settle, then type on the first row.
    let _ = ctx.run(editor_raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
    });
    let mut raw = editor_raw();
    raw.events.push(egui::Event::Text("9".to_string()));
    let _ = ctx.run(raw, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
    });

    assert_eq!(
        app.doc.raw_text, "9{\n  \"a\": 1\n}\n",
        "the keystroke reaches doc.raw_text through the Text tab"
    );
    assert!(app.pending_reparse, "editing schedules the debounced re-parse");
    assert!(app.doc.is_dirty);

    // And the document the tree is built from parses.
    let s = app.settings.clone();
    assert!(app.doc.parse_and_build_tree(true, &s));
    assert!(app.doc.parse_error.is_none());
}

#[test]
fn typing_in_the_split_tab_updates_the_both_editors() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let mut app = ViewerApp::new(Settings::default(), None);
    app.doc.raw_text = "{\n  \"a\": 1\n}\n".to_string();
    app.doc.active_tab = AppTab::Split;
    app.last_tab = AppTab::Split;
    app.split_editor.set_text(&app.doc.raw_text);
    app.editor.set_text(&app.doc.raw_text);

    let mut draw = |app: &mut ViewerApp, ui: &mut egui::Ui| {
        app.show_split_tab(&ctx, ui);
    };

    let _ = ctx.run(editor_raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
    });
    let mut raw = editor_raw();
    raw.events.push(egui::Event::Text("7".to_string()));
    let _ = ctx.run(raw, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
    });

    assert_eq!(app.doc.raw_text, "7{\n  \"a\": 1\n}\n");
    assert_eq!(
        app.split_editor.text(),
        app.doc.raw_text,
        "the split editor is the source"
    );
    assert_eq!(
        app.editor.text(),
        app.doc.raw_text,
        "the Text tab editor mirrors the split editor"
    );
}

/// The editor positions the caret with its own arithmetic instead of asking
/// egui where the character is. If those two disagree, the caret drifts left of
/// the text and the error grows with the column. This pins the painted caret to
/// egui's own glyph positions.
#[test]
fn caret_lines_up_with_the_text_egui_paints() {
    use jsonviewer::code_editor::HORIZONTAL_PADDING;

    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let font = egui::FontId::monospace(13.0);
    let line = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";

    // egui's own answer for where each character sits inside a painted line.
    let galley = ctx.fonts(|f| {
        f.layout(line.to_owned(), font.clone(), egui::Color32::WHITE, f32::INFINITY)
    });
    let glyph_x = |col: usize| galley.pos_from_cursor(egui::text::CCursor::new(col)).min.x;

    // Put the caret partway along the line so the error would be obvious.
    let col = 30;
    let mut editor = CodeEditor::new(format!("{line}\n").as_str());
    editor.request_focus();
    let mut raw = editor_raw();
    raw.events.push(key(egui::Key::End));
    for _ in 0..1 {
        let _ = ctx.run(raw.clone(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                editor.show(ui, font.clone());
            });
        });
    }
    // `End` puts the caret at the end; walk left to the column under test.
    let mut walks = Vec::new();
    for _ in 0..(line.chars().count() - col) {
        walks.push(key(egui::Key::ArrowLeft));
    }
    let _ = ctx.run(raw.clone(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, font.clone());
        });
    });
    let mut walk_raw = raw.clone();
    walk_raw.events = walks;
    let out = ctx.run(walk_raw, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, font.clone());
        });
    });
    assert_eq!(
        editor.cursor().col,
        col,
        "caret should be at the column under test"
    );

    // The painted caret is the only narrow, tall rect near the first row.
    let mut caret_x = None;
    let mut mesh_clip_x = None;
    for clipped in &out.shapes {
        match &clipped.shape {
            egui::Shape::Rect(r) => {
                let r = r.rect;
                if (r.width() - 2.0).abs() < 0.75 && r.height() > 3.0 && r.min.y < 60.0 {
                    caret_x = Some(r.min.x);
                }
            }
            egui::Shape::Mesh(_) => {
                mesh_clip_x.get_or_insert(clipped.clip_rect.min.x);
            }
            _ => {}
        }
    }
    let caret_x = caret_x.expect("a caret rect was painted");
    let origin_x = mesh_clip_x.expect("text mesh was painted");
    let expected = origin_x + HORIZONTAL_PADDING + glyph_x(col);
    assert!(
        (caret_x - expected).abs() < 1.0,
        "caret at x={caret_x} but character {col} is at x={expected} (off by {:+.2})",
        caret_x - expected
    );
}

#[test]
fn cell_metrics_match_what_egui_actually_uses() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);
    let font = egui::FontId::monospace(13.0);
    let mut editor = CodeEditor::new("0123456789ABCDEFGHIJ");

    // egui snaps the advance and the row height to whole pixels, so the editor
    // must not use the raw values: doing so puts the caret left of the text.
    let raw_advance = ctx.fonts(|f| f.glyph_width(&font, '0'));
    let raw_row = ctx.fonts(|f| f.row_height(&font));
    let galley = ctx.fonts(|f| {
        f.layout("0123456789".to_owned(), font.clone(), egui::Color32::WHITE, f32::INFINITY)
    });
    let actual_advance = galley.pos_from_cursor(egui::text::CCursor::new(10)).min.x / 10.0;
    let actual_row = galley.rect.height();

    assert_ne!(
        raw_advance, actual_advance,
        "precondition: the raw advance really is unrounded"
    );

    // A rendered frame must place the caret using the rounded numbers.
    let out = ctx.run(editor_raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            editor.show(ui, font.clone());
        });
    });
    let mut mesh_clip_x = None;
    for clipped in &out.shapes {
        if let egui::Shape::Mesh(_) = &clipped.shape {
            mesh_clip_x.get_or_insert(clipped.clip_rect.min.x);
        }
    }
    assert!(mesh_clip_x.is_some(), "text was painted");
    assert!(
        (actual_row - raw_row).abs() > 0.001,
        "precondition: the raw row height really is unrounded"
    );
}

#[test]
fn emptying_the_document_drops_the_stale_tree() {
    let s = Settings::default();
    let mut m = DocumentModel::new(&s);
    m.raw_text = r#"{"a":1,"b":[1,2,3]}"#.to_string();
    assert!(m.parse_and_build_tree(true, &s));
    m.expand_all();
    assert!(m.root.is_some());
    assert!(!m.visible_tree_rows.is_empty());

    // Break the JSON, then clear it completely. The status bar must stop
    // reporting the old error and the tree must be gone, not left stale.
    m.raw_text = "{ \"a\": ".to_string();
    m.mark_edited();
    assert!(!m.parse_and_build_tree(true, &s));
    assert!(m.parse_error.is_some(), "invalid text reports the error");
    assert!(m.root.is_some(), "a non-empty invalid document keeps the last tree");

    m.raw_text = String::new();
    m.mark_edited();
    assert!(!m.parse_and_build_tree(true, &s));
    assert!(m.root.is_none(), "an empty document has no tree to keep");
    assert!(m.json_value.is_none());
    assert!(m.visible_tree_rows.is_empty());
    assert!(m.selected_id.is_none());
    assert!(
        m.parse_error.is_none(),
        "the stale parse error must be cleared, not shown for an empty document"
    );
    assert_eq!(m.status_text(), "Ready");

    // Whitespace only behaves the same way.
    m.raw_text = r#"{"a":1}"#.to_string();
    assert!(m.parse_and_build_tree(true, &s));
    assert!(m.root.is_some());
    m.raw_text = "   \n\t  ".to_string();
    m.mark_edited();
    assert!(!m.parse_and_build_tree(true, &s));
    assert!(m.root.is_none());
    assert_eq!(m.status_text(), "Ready");
}

#[test]
fn editing_a_large_document_stays_interactive() {
    let ctx = egui::Context::default();
    setup_fonts(&ctx);

    // ~1.5 MB. Editing must not rescan the document, so a keystroke stays in
    // single-digit milliseconds even though the file is far past what a text
    // widget should have to redo per keypress.
    let mut text = String::from("{\n  \"records\": [\n");
    for i in 0..20_000 {
        if i > 0 {
            text.push_str(",\n");
        }
        text.push_str(&format!("    {{\"id\": {i}, \"name\": \"name_{i}\"}}}}"));
    }
    text.push_str("\n  ]\n}");

    let mut app = ViewerApp::new(Settings::default(), None);
    app.doc.raw_text = text.clone();
    app.doc.update_metrics();
    app.doc.active_tab = AppTab::Text;
    app.last_tab = AppTab::Text;
    app.editor.set_text(&app.doc.raw_text);

    let mut draw = |app: &mut ViewerApp, ui: &mut egui::Ui| {
        app.show_text_tab(&ctx, ui);
    };
    let _ = ctx.run(editor_raw(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
    });

    // Type a character near the end of the document, where a rescan of the
    // remaining text would be cheapest, and again near the start, where it
    // would be most expensive.
    for (name, downs) in [("near the end", 0usize), ("near the start", 19_000)] {
        for _ in 0..downs {
            let _ = ctx.run(editor_raw(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
            });
        }
        let start = std::time::Instant::now();
        let mut raw = editor_raw();
        raw.events.push(egui::Event::Text("q".to_string()));
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(&mut app, ui));
        });
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        assert!(ms < 16.0, "keystroke {name} in a 1.5 MB document: {ms:.2} ms");
    }
    assert!(app.doc.raw_text.contains('q'), "the keystroke landed");
}
