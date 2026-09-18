//! Document model — Rust port of `Sources/JSONViewerCore/JSONDocumentModel.swift`
//! plus `JSONNode.swift` tree helpers. UI-agnostic: the Win32 layer in
//! `app.rs` owns a `DocumentModel` and mirrors its state into native controls.

use crate::json::{unescape_stringified_json, JSONParseError, JSONParser, JSONValue};
use crate::python::parse_python_literal;
use crate::settings::Settings;

pub const SAMPLE_JSON: &str = "{\n  \"title\": \"JSON Viewer macOS\",\n  \"version\": \"1.0.0\",\n  \"description\": \"Native Mac JSON Viewer & Formatter\",\n  \"active\": true,\n  \"rating\": 4.95,\n  \"nullProperty\": null,\n  \"author\": {\n    \"name\": \"Antigravity & Stack.hu\",\n    \"email\": \"local@mac.internal\"\n  },\n  \"features\": [\n    \"Hierarchical Tree View\",\n    \"Property Grid Inspection\",\n    \"2-Space Indented Formatting\",\n    \"Whitespace Minification\",\n    \"Remote URL Loading\",\n    \"Full Key & Value Search\"\n  ],\n  \"statistics\": {\n    \"downloads\": 12840,\n    \"stars\": 892\n  }\n}";

#[derive(Clone, Debug)]
pub struct Node {
    pub key: String,
    pub value: JSONValue,
    pub path: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn id(&self) -> &str {
        &self.path
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn is_container(&self) -> bool {
        !self.is_leaf()
    }

    pub fn display_text(&self) -> String {
        match &self.value {
            JSONValue::Null => format!("{} : null", self.key),
            JSONValue::Bool(b) => format!("{} : {}", self.key, if *b { "true" } else { "false" }),
            JSONValue::Number(_, raw) => format!("{} : {}", self.key, raw),
            JSONValue::Str(s) => format!("{} : \"{}\"", self.key, s),
            JSONValue::Array(items) => {
                if self.key == "JSON" {
                    format!("JSON [{}]", items.len())
                } else {
                    format!("{} [{}]", self.key, items.len())
                }
            }
            JSONValue::Object(pairs) => {
                if self.key == "JSON" {
                    format!("JSON {{{}}}", pairs.len())
                } else {
                    format!("{} {{{}}}", self.key, pairs.len())
                }
            }
        }
    }

    pub fn tree_label(&self) -> String {
        // Compact single-line label for the Win32 TreeView control.
        // Leaf text is truncated for display (like the mac collapsed rows);
        // the full value stays available for copy / expand.
        if self.is_container() {
            let badge = match &self.value {
                JSONValue::Array(items) => format!("[{}]", items.len()),
                JSONValue::Object(pairs) => format!("{{{}}}", pairs.len()),
                _ => String::new(),
            };
            if self.key == "JSON" {
                format!("JSON {}", badge)
            } else {
                format!("{} {}", self.key, badge)
            }
        } else {
            truncate_chars(&self.display_text(), 160)
        }
    }

    pub fn value_string(&self) -> String {
        match &self.value {
            JSONValue::Null => "null".to_string(),
            JSONValue::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
            JSONValue::Number(_, raw) => raw.clone(),
            JSONValue::Str(s) => s.clone(),
            JSONValue::Array(_) => format!("[{} items]", self.children.len()),
            JSONValue::Object(_) => format!("{{{} properties}}", self.children.len()),
        }
    }

    /// Short display form for the property grid (mac parity: grid cells show
    /// a preview; the full value is one click/copy away).
    pub fn value_string_short(&self) -> String {
        let full = self.value_string();
        let count = full.chars().count();
        if count > 200 {
            let head: String = full.chars().take(200).collect();
            format!("{}… ({} chars)", head, count)
        } else {
            full
        }
    }

    /// Full-length leaf text (for the Expand dialog / copy actions).
    pub fn is_long_text(&self) -> bool {
        match &self.value {
            JSONValue::Str(s) => s.chars().count() > 160 || s.contains('\n'),
            _ => false,
        }
    }

    pub fn formatted_value_string(&self) -> String {
        match &self.value {
            JSONValue::Null => "null".to_string(),
            JSONValue::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
            JSONValue::Number(_, raw) => raw.clone(),
            JSONValue::Str(s) => format!("\"{}\"", s),
            JSONValue::Array(_) | JSONValue::Object(_) => self.value.format(2, false, false),
        }
    }

    pub fn type_badge(&self) -> String {
        match &self.value {
            JSONValue::Str(_) => "str".to_string(),
            JSONValue::Number(_, _) => "num".to_string(),
            JSONValue::Bool(_) => "bool".to_string(),
            JSONValue::Null => "null".to_string(),
            JSONValue::Array(items) => format!("[{}]", items.len()),
            JSONValue::Object(pairs) => format!("{{{}}}", pairs.len()),
        }
    }

    pub fn build_tree(value: &JSONValue, root_key: &str) -> Node {
        build_node(root_key, value, "$")
    }
}

fn build_node(key: &str, value: &JSONValue, path: &str) -> Node {
    match value {
        JSONValue::Object(pairs) => {
            let children = pairs
                .iter()
                .map(|p| {
                    let child_path = format!("{}.{}", path, p.key);
                    build_node(&p.key, &p.value, &child_path)
                })
                .collect();
            Node { key: key.to_string(), value: value.clone(), path: path.to_string(), children }
        }
        JSONValue::Array(items) => {
            let children = items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let child_path = format!("{}[{}]", path, i);
                    build_node(&i.to_string(), item, &child_path)
                })
                .collect();
            Node { key: key.to_string(), value: value.clone(), path: path.to_string(), children }
        }
        _ => Node { key: key.to_string(), value: value.clone(), path: path.to_string(), children: Vec::new() },
    }
}

#[derive(Clone, Debug)]
pub struct PropertyRow {
    pub name: String,
    pub value: String,
    pub typ: String,
    pub path: String,
}

/// Char-boundary-safe truncation with an ellipsis marker.
/// Single-pass: avoids counting the whole string twice (which was O(n) x2
/// per tree row — painful for MB-sized leaf strings).
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut count = 0usize;
    let mut end = s.len();
    for (i, _) in s.char_indices() {
        if count == max_chars {
            end = i;
            break;
        }
        count += 1;
    }
    if end == s.len() {
        // Fits (count <= max_chars).
        return s.to_string();
    }
    let mut out = String::with_capacity(end + 3);
    out.push_str(&s[..end]);
    out.push('…');
    out
}

/// One pre-order row of the tree, pre-rendered for fast chunked insertion
/// into the native TreeView (keeps the UI responsive on huge files).
#[derive(Clone, Debug)]
pub struct FlatRow {
    pub path: String,
    pub label: String,
    pub depth: usize,
    pub is_container: bool,
}

/// Flatten the tree in pre-order (iterative — safe for any depth).
pub fn flatten(root: &Node) -> Vec<FlatRow> {
    let mut out = Vec::new();
    let mut stack: Vec<(&Node, usize)> = vec![(root, 0)];
    while let Some((node, depth)) = stack.pop() {
        out.push(FlatRow {
            path: node.path.clone(),
            label: node.tree_label(),
            depth,
            is_container: node.is_container(),
        });
        for child in node.children.iter().rev() {
            stack.push((child, depth + 1));
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppTab {
    Viewer,
    Text,
    Split,
}

impl AppTab {
    pub fn from_settings(s: &str) -> Self {
        match s {
            "Viewer" => AppTab::Viewer,
            "Split" => AppTab::Split,
            _ => AppTab::Text,
        }
    }
}

pub struct DocumentModel {
    pub raw_text: String,
    pub json_value: Option<JSONValue>,
    pub root: Option<Node>,
    pub selected_path: Option<String>,
    pub parse_error: Option<JSONParseError>,
    pub error_message: String,
    pub search_query: String,
    pub search_results: Vec<String>,
    pub current_search_index: usize,
    pub search_status: String,
    pub last_executed_query: String,
    pub character_count: usize,
    pub line_count: usize,
    pub is_dirty: bool,
    pub active_tab: AppTab,
}

impl DocumentModel {
    pub fn new(settings: &Settings) -> Self {
        let mut m = Self {
            raw_text: SAMPLE_JSON.to_string(),
            json_value: None,
            root: None,
            selected_path: None,
            parse_error: None,
            error_message: String::new(),
            search_query: String::new(),
            search_results: Vec::new(),
            current_search_index: 0,
            search_status: String::new(),
            last_executed_query: String::new(),
            character_count: 0,
            line_count: 1,
            is_dirty: false,
            active_tab: AppTab::from_settings(&settings.default_tab),
        };
        // The sample must always open properly formatted (beautified per
        // current settings), never as a single packed line. Parse first so
        // we know it is valid, then canonicalize whitespace. The tree stays
        // valid — only whitespace changed — so no re-parse is needed.
        let _ = m.parse_and_build_tree(true, settings);
        if let Some(v) = m.json_value.clone() {
            let pretty = v.format(
                settings.indent_spaces,
                settings.sort_keys_alphabetically,
                settings.escape_slashes_in_stringify,
            );
            if !pretty.is_empty() {
                m.raw_text = pretty;
            }
        }
        m.update_metrics();
        m.is_dirty = false;
        m
    }

    pub fn update_metrics(&mut self) {
        if self.raw_text.is_empty() {
            self.character_count = 0;
            self.line_count = 1;
            return;
        }
        // Fast path: JSON is overwhelmingly ASCII, where UTF-16 units ==
        // byte length. `is_ascii()` is SIMD-fast, much cheaper than
        // `encode_utf16().count()` which decodes every scalar
        // (~9ms vs ~3ms on a 5MB file in benchmarks).
        if self.raw_text.is_ascii() {
            self.character_count = self.raw_text.len();
        } else {
            self.character_count = self.raw_text.encode_utf16().count();
        }
        self.line_count = 1 + self.raw_text.bytes().filter(|&b| b == b'\n').count();
    }

    pub fn mark_edited(&mut self) {
        self.is_dirty = true;
        self.update_metrics();
    }

    pub fn parse_and_build_tree(&mut self, silent: bool, settings: &Settings) -> bool {
        // Borrow the trimmed view — no 5MB copy. `JSONParser::parse` returns
        // owned values, so the borrow ends before we mutate `raw_text` below
        // (NLL). The old `trim().to_string()` duplicated the whole file on
        // every parse.
        let trimmed = self.raw_text.trim();
        if trimmed.is_empty() {
            if !silent {
                self.error_message = "JSON error: Please enter JSON code in the Text tab first.".to_string();
            }
            return false;
        }
        let mut auto_converted: Option<String> = None;
        let parsed: Result<JSONValue, JSONParseError> = match JSONParser::parse(trimmed) {
            Ok(v) => Ok(v),
            Err(initial) => {
                if let Ok(py) = parse_python_literal(trimmed) {
                    auto_converted = Some(py.format(settings.indent_spaces, settings.sort_keys_alphabetically, settings.escape_slashes_in_stringify));
                    Ok(py)
                } else if settings.auto_unwrap_stringified {
                    let unescaped = unescape_stringified_json(trimmed);
                    if unescaped != trimmed {
                        if let Ok(fb) = JSONParser::parse(&unescaped) {
                            Ok(fb)
                        } else if let Ok(fb_py) = parse_python_literal(&unescaped) {
                            auto_converted = Some(fb_py.format(settings.indent_spaces, settings.sort_keys_alphabetically, settings.escape_slashes_in_stringify));
                            Ok(fb_py)
                        } else {
                            Err(initial)
                        }
                    } else {
                        Err(initial)
                    }
                } else {
                    Err(initial)
                }
            }
        };
        if let Some(converted) = auto_converted {
            self.raw_text = converted;
            self.update_metrics();
        }

        let mut parsed = match parsed {
            Ok(v) => v,
            Err(e) => {
                self.parse_error = Some(e.clone());
                if !silent {
                    self.error_message = format!("JSON error: Invalid JSON variable\n\n{} at line {}, col {}", e.message, e.line, e.column);
                }
                return false;
            }
        };

        if settings.auto_unwrap_stringified {
            if let JSONValue::Str(inner) = &parsed {
                let t = inner.trim();
                if (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']')) {
                    if let Ok(inner_parsed) = JSONParser::parse(t) {
                        parsed = inner_parsed;
                    }
                }
            }
        }

        let root = Node::build_tree(&parsed, "JSON");
        self.selected_path = Some(root.path.clone());
        self.json_value = Some(parsed);
        self.root = Some(root);
        self.parse_error = None;
        self.is_dirty = false;
        true
    }

    pub fn clear(&mut self) {
        self.raw_text.clear();
        self.json_value = None;
        self.root = None;
        self.selected_path = None;
        self.parse_error = None;
        self.clear_search();
        self.update_metrics();
        self.is_dirty = false;
    }

    // MARK: - Transformations (return error string on failure)

    fn parse_for_transform(&self) -> Result<JSONValue, String> {
        let trimmed = self.raw_text.trim();
        if let Ok(v) = JSONParser::parse(trimmed) {
            return Ok(v);
        }
        if let Ok(v) = parse_python_literal(trimmed) {
            return Ok(v);
        }
        match JSONParser::parse(trimmed) {
            Ok(v) => Ok(v),
            Err(e) => Err(format!("Invalid JSON or Python Object ({} at line {}, col {})", e.message, e.line, e.column)),
        }
    }

    pub fn beautify(&mut self, settings: &Settings) -> Result<(), String> {
        if self.raw_text.trim().is_empty() {
            return Ok(());
        }
        let mut val = self.parse_for_transform()?;
        if settings.auto_unwrap_stringified {
            if let JSONValue::Str(s) = &val {
                let t = s.trim();
                if let Ok(u) = JSONParser::parse(t) {
                    val = u;
                } else if let Ok(u) = parse_python_literal(t) {
                    val = u;
                }
            }
        }
        self.raw_text = val.format(settings.indent_spaces, settings.sort_keys_alphabetically, settings.escape_slashes_in_stringify);
        self.update_metrics();
        let _ = self.parse_and_build_tree(true, settings);
        Ok(())
    }

    pub fn minify(&mut self, settings: &Settings) -> Result<(), String> {
        if self.raw_text.trim().is_empty() {
            return Ok(());
        }
        match JSONParser::parse(self.raw_text.trim()) {
            Ok(val) => {
                self.raw_text = val.minify(settings.escape_slashes_in_stringify);
                self.update_metrics();
                let _ = self.parse_and_build_tree(true, settings);
                Ok(())
            }
            Err(e) => Err(format!("Invalid JSON ({} at line {}, col {})", e.message, e.line, e.column)),
        }
    }

    pub fn stringify(&mut self, settings: &Settings) {
        let trimmed = self.raw_text.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Ok(val) = JSONParser::parse(trimmed) {
            self.raw_text = val.stringify(settings.escape_slashes_in_stringify);
        } else {
            self.raw_text = crate::json::quote_and_escape_string(&self.raw_text, settings.escape_slashes_in_stringify);
        }
        self.update_metrics();
        let _ = self.parse_and_build_tree(true, settings);
    }

    pub fn unescape(&mut self, settings: &Settings) {
        self.raw_text = unescape_stringified_json(&self.raw_text);
        self.update_metrics();
        let _ = self.parse_and_build_tree(true, settings);
    }

    pub fn json_to_python(&mut self, settings: &Settings) -> Result<(), String> {
        if self.raw_text.trim().is_empty() {
            return Ok(());
        }
        let val = self.parse_for_transform()?;
        let indent = if settings.indent_spaces < 0 { 4 } else { settings.indent_spaces as usize };
        self.raw_text = val.to_python_object(indent);
        self.update_metrics();
        Ok(())
    }

    pub fn python_to_json(&mut self, settings: &Settings) -> Result<(), String> {
        if self.raw_text.trim().is_empty() {
            return Ok(());
        }
        match parse_python_literal(self.raw_text.trim()) {
            Ok(val) => {
                self.raw_text = val.format(settings.indent_spaces, settings.sort_keys_alphabetically, false);
                self.update_metrics();
                let _ = self.parse_and_build_tree(true, settings);
                Ok(())
            }
            Err(e) => Err(format!("Invalid Python dictionary syntax ({})", e)),
        }
    }

    // MARK: - Copy variants (pure string builders)

    fn current_value(&self) -> Option<JSONValue> {
        if let Some(v) = &self.json_value {
            return Some(v.clone());
        }
        let t = self.raw_text.trim();
        if t.is_empty() {
            return None;
        }
        JSONParser::parse(t).ok().or_else(|| parse_python_literal(t).ok())
    }

    pub fn copy_text(&self) -> String {
        self.raw_text.clone()
    }

    pub fn copy_beautified(&self, settings: &Settings) -> String {
        if let Some(v) = self.current_value() {
            v.format(settings.indent_spaces, settings.sort_keys_alphabetically, false)
        } else {
            self.raw_text.clone()
        }
    }

    pub fn copy_minified(&self) -> String {
        if let Some(v) = self.current_value() {
            v.minify(false)
        } else {
            self.raw_text.clone()
        }
    }

    pub fn copy_stringified(&self, settings: &Settings) -> String {
        if let Some(v) = self.current_value() {
            v.stringify(settings.escape_slashes_in_stringify)
        } else {
            crate::json::quote_and_escape_string(&self.raw_text, settings.escape_slashes_in_stringify)
        }
    }

    pub fn copy_python(&self, settings: &Settings) -> String {
        if let Some(v) = self.current_value() {
            let indent = if settings.indent_spaces < 0 { 4 } else { settings.indent_spaces as usize };
            v.to_python_object(indent)
        } else {
            self.raw_text.clone()
        }
    }

    pub fn paste(&mut self, text: &str, settings: &Settings) -> &'static str {
        let trimmed = text.trim();
        if JSONParser::parse(trimmed).is_err() {
            if let Ok(py) = parse_python_literal(trimmed) {
                self.raw_text = py.format(settings.indent_spaces, settings.sort_keys_alphabetically, false);
                self.update_metrics();
                let _ = self.parse_and_build_tree(true, settings);
                return "converted-python";
            }
        } else if has_huge_line(trimmed, 8000) {
            // Pasted minified blob: beautify once so the EDIT control holds
            // many short lines instead of one MB-sized line (typing perf).
            if let Ok(v) = JSONParser::parse(trimmed) {
                self.raw_text = v.format(
                    settings.indent_spaces,
                    settings.sort_keys_alphabetically,
                    settings.escape_slashes_in_stringify,
                );
                self.update_metrics();
                let _ = self.parse_and_build_tree(true, settings);
                return "pasted";
            }
        }
        self.raw_text = text.to_string();
        self.update_metrics();
        let _ = self.parse_and_build_tree(true, settings);
        "pasted"
    }

    // MARK: - Tree queries

    pub fn find_node(&self, path: &str) -> Option<&Node> {
        self.root.as_ref().and_then(|r| find_in(r, path))
    }

    pub fn find_with_ancestors(&self, path: &str) -> Option<(Vec<String>, bool)> {
        // Returns (ancestor paths from root to parent, found).
        let mut ancestors = Vec::new();
        let found = collect_ancestors(self.root.as_ref()?, path, &mut ancestors);
        if found { Some((ancestors, true)) } else { None }
    }

    pub fn properties_for_selected(&self) -> Vec<PropertyRow> {
        let sel = match &self.selected_path {
            Some(p) => p.clone(),
            None => return Vec::new(),
        };
        let root = match &self.root {
            Some(r) => r,
            None => return Vec::new(),
        };
        let node = match find_in(root, &sel) {
            Some(n) => n,
            None => return Vec::new(),
        };
        // Match Swift: if leaf, show parent's properties.
        let container: &Node = if node.is_container() {
            node
        } else {
            match find_parent(root, &sel) {
                Some(p) => p,
                None => {
                    return vec![PropertyRow {
                        name: node.key.clone(),
                        value: node.value_string_short(),
                        typ: node.value.type_name().to_string(),
                        path: node.path.clone(),
                    }];
                }
            }
        };
        if container.children.is_empty() {
            return vec![PropertyRow {
                name: container.key.clone(),
                value: container.value_string_short(),
                typ: container.value.type_name().to_string(),
                path: container.path.clone(),
            }];
        }
        container.children.iter().map(|c| PropertyRow {
            name: c.key.clone(),
            value: c.value_string_short(),
            typ: c.value.type_name().to_string(),
            path: c.path.clone(),
        }).collect()
    }

    pub fn status_text(&self) -> String {
        if let Some(e) = &self.parse_error {
            format!("Error: {} (Line {}, Col {})", e.message, e.line, e.column)
        } else if !self.raw_text.is_empty() {
            "Valid JSON".to_string()
        } else {
            "Ready".to_string()
        }
    }

    pub fn metrics_text(&self) -> String {
        format!("{} lines, {} characters", self.line_count, self.character_count)
    }

    // MARK: - Search

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.search_status.clear();
        self.last_executed_query.clear();
        self.current_search_index = 0;
    }

    pub fn search_start(&mut self, settings: &Settings) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.clear_search();
            return;
        }
        self.last_executed_query = query.clone();
        if self.root.is_none() {
            if !self.parse_and_build_tree(true, settings) {
                self.search_status = "Phrase not found!".to_string();
                return;
            }
        }
        let root = match &self.root {
            Some(r) => r,
            None => {
                self.search_status = "Phrase not found!".to_string();
                return;
            }
        };
        // Borrow the tree — never clone it (it can hold multi-MB strings).
        let matches = search_in(root, &query);
        self.search_results = matches;
        if self.search_results.is_empty() {
            self.search_status = "Phrase not found!".to_string();
        } else {
            self.current_search_index = 0;
            let target = self.search_results[0].clone();
            self.selected_path = Some(target);
            self.search_status = format!("1 of {} matches", self.search_results.len());
        }
    }

    pub fn search_next(&mut self, settings: &Settings) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        if query != self.last_executed_query || self.search_results.is_empty() {
            self.search_start(settings);
            return;
        }
        if self.search_results.is_empty() {
            return;
        }
        self.current_search_index = (self.current_search_index + 1) % self.search_results.len();
        let target = self.search_results[self.current_search_index].clone();
        self.selected_path = Some(target);
        self.search_status = format!("{} of {} matches", self.current_search_index + 1, self.search_results.len());
    }

    pub fn search_previous(&mut self, settings: &Settings) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        if query != self.last_executed_query || self.search_results.is_empty() {
            self.search_start(settings);
            if !self.search_results.is_empty() {
                self.current_search_index = self.search_results.len() - 1;
                let target = self.search_results[self.current_search_index].clone();
                self.selected_path = Some(target);
                self.search_status = format!("{} of {} matches", self.current_search_index + 1, self.search_results.len());
            }
            return;
        }
        if self.search_results.is_empty() {
            return;
        }
        if self.current_search_index == 0 {
            self.current_search_index = self.search_results.len() - 1;
        } else {
            self.current_search_index -= 1;
        }
        let target = self.search_results[self.current_search_index].clone();
        self.selected_path = Some(target);
        self.search_status = format!("{} of {} matches", self.current_search_index + 1, self.search_results.len());
    }

    // MARK: - File I/O

    pub fn load_file(&mut self, path: &std::path::Path, settings: &Settings) -> Result<(), String> {
        match std::fs::read(path) {
            Ok(data) => match String::from_utf8(data) {
                Ok(s) => {
                    self.raw_text = s;
                    // Win32 EDIT controls collapse on MB-sized single lines
                    // (every keystroke re-lays-out the giant line + HS scroll).
                    // If the file parses and has a pathological long line,
                    // beautify it once so the editor holds many short lines.
                    // This is also what users mean by "properly formatted".
                    if has_huge_line(&self.raw_text, 8000) {
                        if let Ok(v) = JSONParser::parse(self.raw_text.trim()) {
                            let pretty = v.format(
                                settings.indent_spaces,
                                settings.sort_keys_alphabetically,
                                settings.escape_slashes_in_stringify,
                            );
                            if !pretty.is_empty() {
                                self.raw_text = pretty;
                            }
                        }
                    }
                    self.update_metrics();
                    let _ = self.parse_and_build_tree(true, settings);
                    Ok(())
                }
                Err(e) => Err(format!("Failed to open file: {}", e)),
            },
            Err(e) => Err(format!("Failed to open file: {}", e)),
        }
    }

    pub fn save_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        match std::fs::write(path, &self.raw_text) {
            Ok(_) => {
                self.is_dirty = false;
                Ok(())
            }
            Err(e) => Err(format!("Failed to save file: {}", e)),
        }
    }
}

fn find_in<'a>(node: &'a Node, path: &str) -> Option<&'a Node> {
    if node.path == path {
        return Some(node);
    }
    for child in &node.children {
        if let Some(f) = find_in(child, path) {
            return Some(f);
        }
    }
    None
}

fn find_parent<'a>(node: &'a Node, path: &str) -> Option<&'a Node> {
    for child in &node.children {
        if child.path == path {
            return Some(node);
        }
        if let Some(f) = find_parent(child, path) {
            return Some(f);
        }
    }
    None
}

fn collect_ancestors(node: &Node, target: &str, stack: &mut Vec<String>) -> bool {
    if node.path == target {
        return true;
    }
    for child in &node.children {
        stack.push(node.path.clone());
        if collect_ancestors(child, target, stack) {
            return true;
        }
        stack.pop();
    }
    false
}

/// True if any line exceeds `limit` chars (single-pass, no allocation).
/// Used to detect minified MB-sized single-line files that kill the EDIT
/// control (see `load_file`).
fn has_huge_line(s: &str, limit: usize) -> bool {
    let mut count = 0usize;
    for ch in s.chars() {
        if ch == '\n' {
            count = 0;
        } else {
            count += 1;
            if count > limit {
                return true;
            }
        }
    }
    false
}

fn search_in(node: &Node, query: &str) -> Vec<String> {
    let q = query.to_lowercase();
    let q_ascii = q.is_ascii();
    let mut out = Vec::new();
    search_recursive(node, &q, q_ascii, &mut out);
    out
}

/// Case-insensitive substring check without allocating a lowercased
/// haystack when the query is ASCII (the common case): ASCII-lower the
/// haystack on the fly via bytes.
fn haystack_contains(haystack: &str, needle_lower: &str, needle_is_ascii: bool) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if needle_is_ascii {
        if haystack.is_ascii() {
            // Both ASCII: byte-wise case-insensitive search, no alloc.
            let n = needle_lower.as_bytes();
            let h = haystack.as_bytes();
            if n.len() > h.len() {
                return false;
            }
            // Lowercase needle is already lower; compare lowercased hay bytes.
            for w in h.windows(n.len()) {
                let mut ok = true;
                for (a, b) in w.iter().zip(n.iter()) {
                    if a.to_ascii_lowercase() != *b {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    return true;
                }
            }
            return false;
        }
        // Non-ASCII haystack + ASCII needle: fall back to lowercase alloc
        // (rare; correctness over speed).
        return haystack.to_lowercase().contains(needle_lower);
    }
    haystack.to_lowercase().contains(needle_lower)
}

fn node_matches_search(node: &Node, q: &str, q_ascii: bool) -> bool {
    // Check cheapest fields first with early exit — avoids building the
    // full `display_text()` string (which copies MB-sized leaf values)
    // for every node on every search.
    if haystack_contains(&node.key, q, q_ascii) {
        return true;
    }
    if haystack_contains(&node.path, q, q_ascii) {
        return true;
    }
    match &node.value {
        JSONValue::Null => haystack_contains("null", q, q_ascii),
        JSONValue::Bool(b) => {
            haystack_contains(if *b { "true" } else { "false" }, q, q_ascii)
        }
        JSONValue::Number(_, raw) => haystack_contains(raw, q, q_ascii),
        JSONValue::Str(s) => haystack_contains(s, q, q_ascii),
        JSONValue::Array(items) => {
            // Match container badge text like "key [12]" without formatting.
            // Key/path already checked; only match the count form if the
            // query looks numeric-ish. Cheap fallback: format tiny badge.
            let badge = format!("[{}]", items.len());
            haystack_contains(&badge, q, q_ascii)
        }
        JSONValue::Object(pairs) => {
            let badge = format!("{{{}}}", pairs.len());
            haystack_contains(&badge, q, q_ascii)
        }
    }
}

fn search_recursive(node: &Node, q: &str, q_ascii: bool, out: &mut Vec<String>) {
    // Iterative pre-order (safe for pathological depth; recursion could
    // stack-overflow on 10k-deep JSON).
    let mut stack: Vec<&Node> = vec![node];
    while let Some(n) = stack.pop() {
        if node_matches_search(n, q, q_ascii) {
            out.push(n.path.clone());
        }
        for child in n.children.iter().rev() {
            stack.push(child);
        }
    }
}
