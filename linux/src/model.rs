//! Document model — Rust port of `Sources/JSONViewerCore/JSONDocumentModel.swift`
//! plus `JSONNode.swift` tree helpers. UI-agnostic model providing:
//! - Order-preserving JSON parsing and Python-dictionary conversions
//! - Hierarchical node tree with `Arc<Node>` zero-copy sharing
//! - Flattened virtualized tree row index (`visible_tree_rows`) for 144Hz smooth rendering
//! - Bidirectional property inspection and parent drill-down navigation
//! - Substring and JSON path search with instant match highlighting

use crate::json::{unescape_stringified_json, JSONParseError, JSONParser, JSONValue};
use crate::python::parse_python_literal;
use crate::settings::Settings;
use std::collections::HashSet;
use std::sync::Arc;

pub const SAMPLE_JSON: &str = "{\n  \"title\": \"JSON Viewer macOS\",\n  \"version\": \"1.0.0\",\n  \"description\": \"Native Mac JSON Viewer & Formatter\",\n  \"active\": true,\n  \"rating\": 4.95,\n  \"nullProperty\": null,\n  \"author\": {\n    \"name\": \"Antigravity & Stack.hu\",\n    \"email\": \"local@mac.internal\"\n  },\n  \"features\": [\n    \"Hierarchical Tree View\",\n    \"Property Grid Inspection\",\n    \"2-Space Indented Formatting\",\n    \"Whitespace Minification\",\n    \"Remote URL Loading\",\n    \"Full Key & Value Search\"\n  ],\n  \"statistics\": {\n    \"downloads\": 12840,\n    \"stars\": 892\n  }\n}";

#[derive(Clone, Debug)]
pub struct Node {
    pub key: String,
    pub value: JSONValue,
    pub path: String,
    pub children: Vec<Node>,
}

#[allow(dead_code)]
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

    /// Short display form for the property grid & tree leaf preview.
    pub fn value_string_short(&self) -> String {
        match &self.value {
            JSONValue::Str(s) => {
                if s.contains('\n') || s.chars().count() > 80 {
                    let first_line = s.lines().next().unwrap_or("");
                    let count = s.chars().count();
                    if first_line.chars().count() > 80 {
                        let head: String = first_line.chars().take(80).collect();
                        format!("\"{}…\" ({} chars)", head, count)
                    } else {
                        format!("\"{}\"… ({} chars)", first_line, count)
                    }
                } else {
                    format!("\"{}\"", s)
                }
            }
            JSONValue::Null => "null".to_string(),
            JSONValue::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
            JSONValue::Number(_, raw) => raw.clone(),
            JSONValue::Array(_) => format!("[{} items]", self.children.len()),
            JSONValue::Object(_) => format!("{{{} properties}}", self.children.len()),
        }
    }

    /// True if string value exceeds 45 chars or contains multiple lines.
    pub fn is_long_text(&self) -> bool {
        match &self.value {
            JSONValue::Str(s) => s.chars().count() > 45 || s.contains('\n'),
            _ => false,
        }
    }

    pub fn formatted_value_string(&self) -> String {
        match &self.value {
            JSONValue::Null => "null".to_string(),
            JSONValue::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
            JSONValue::Number(_, raw) => raw.clone(),
            JSONValue::Str(s) => s.clone(),
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

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRow {
    pub name: String,
    pub value: String,
    pub typ: String,
    pub path: String,
    pub is_container: bool,
}

/// Char-boundary-safe truncation with an ellipsis marker.
#[allow(dead_code)]
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
        return s.to_string();
    }
    let mut out = String::with_capacity(end + 3);
    out.push_str(&s[..end]);
    out.push('…');
    out
}

/// Flattened pre-order tree row representation for virtualized rendering.
/// Virtualization renders only rows intersecting the current viewport.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatTreeRow {
    pub path: String,
    pub key: String,
    pub depth: usize,
    pub is_container: bool,
    pub is_expanded: bool,
    pub is_leaf_expanded: bool,
    pub is_match: bool,
    pub badge_text: String,
    pub value_repr: String,
    pub full_str: Option<String>,
    pub char_count: usize,
    pub type_name: &'static str,
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
    pub root: Option<Arc<Node>>,
    pub selected_path: Option<String>,
    pub parse_error: Option<JSONParseError>,
    pub error_message: String,
    pub search_query: String,
    pub search_results: Vec<String>,
    pub search_result_ids: HashSet<String>,
    pub current_search_index: usize,
    pub search_status: String,
    pub last_executed_query: String,
    pub character_count: usize,
    pub line_count: usize,
    pub is_dirty: bool,
    pub active_tab: AppTab,
    pub expanded_nodes: HashSet<String>,
    pub expanded_leaf_nodes: HashSet<String>,
    pub visible_tree_rows: Vec<FlatTreeRow>,
    pub tree_version: usize,
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
            search_result_ids: HashSet::new(),
            current_search_index: 0,
            search_status: String::new(),
            last_executed_query: String::new(),
            character_count: 0,
            line_count: 1,
            is_dirty: false,
            active_tab: AppTab::from_settings(&settings.default_tab),
            expanded_nodes: HashSet::new(),
            expanded_leaf_nodes: HashSet::new(),
            visible_tree_rows: Vec::new(),
            tree_version: 0,
        };
        let _ = m.parse_and_build_tree(true, settings);
        if let Some(v) = &m.json_value {
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
        let root_path = root.path.clone();
        self.selected_path = Some(root_path.clone());
        self.json_value = Some(parsed);
        self.root = Some(Arc::new(root));
        self.parse_error = None;
        self.is_dirty = false;

        // macOS parity: initially ONLY root container is expanded
        self.expanded_nodes.clear();
        self.expanded_nodes.insert(root_path);
        self.update_visible_rows();
        true
    }

    pub fn update_visible_rows(&mut self) {
        self.visible_tree_rows.clear();
        let Some(root) = &self.root else { return };

        let mut stack: Vec<(&Node, usize)> = vec![(root.as_ref(), 0)];
        while let Some((node, depth)) = stack.pop() {
            let is_container = node.is_container();
            let is_expanded = self.expanded_nodes.contains(&node.path);
            let is_leaf_expanded = self.expanded_leaf_nodes.contains(&node.path);
            let is_match = self.search_result_ids.contains(&node.path);
            let is_long = node.is_long_text();
            let (full_str, char_count) = match &node.value {
                JSONValue::Str(s) => (Some(s.clone()), s.chars().count()),
                _ => (None, 0),
            };

            let row = FlatTreeRow {
                path: node.path.clone(),
                key: node.key.clone(),
                depth,
                is_container,
                is_expanded,
                is_leaf_expanded,
                is_match,
                badge_text: node.type_badge(),
                value_repr: node.value_string_short(),
                full_str,
                char_count,
                type_name: node.value.type_name(),
            };
            self.visible_tree_rows.push(row);

            if is_container && is_expanded {
                for child in node.children.iter().rev() {
                    stack.push((child, depth + 1));
                }
            }
            let _ = is_long;
        }
        self.tree_version += 1;
    }

    pub fn toggle_expand(&mut self, path: &str) {
        if self.expanded_nodes.contains(path) {
            self.expanded_nodes.remove(path);
        } else {
            self.expanded_nodes.insert(path.to_string());
        }
        self.update_visible_rows();
    }

    pub fn toggle_expand_leaf(&mut self, path: &str) {
        if self.expanded_leaf_nodes.contains(path) {
            self.expanded_leaf_nodes.remove(path);
        } else {
            self.expanded_leaf_nodes.insert(path.to_string());
        }
        self.update_visible_rows();
    }

    pub fn expand_all(&mut self) {
        if let Some(root) = &self.root {
            let mut stack = vec![root.as_ref()];
            let mut count = 0usize;
            while let Some(n) = stack.pop() {
                if n.is_container() {
                    self.expanded_nodes.insert(n.path.clone());
                    count += 1;
                    if count > 80_000 {
                        break;
                    }
                    for c in &n.children {
                        if c.is_container() {
                            stack.push(c);
                        }
                    }
                }
            }
            self.update_visible_rows();
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded_nodes.clear();
        self.expanded_leaf_nodes.clear();
        if let Some(root) = &self.root {
            self.expanded_nodes.insert(root.path.clone());
        }
        self.update_visible_rows();
    }

    pub fn expand_subtree(&mut self, path: &str) {
        let paths: Vec<String> = if let Some(node) = self.find_node(path) {
            let mut stack = vec![node];
            let mut collected = Vec::new();
            while let Some(n) = stack.pop() {
                if n.is_container() {
                    collected.push(n.path.clone());
                    for c in &n.children {
                        if c.is_container() {
                            stack.push(c);
                        }
                    }
                }
            }
            collected
        } else {
            Vec::new()
        };
        for p in paths {
            self.expanded_nodes.insert(p);
        }
        self.update_visible_rows();
    }

    pub fn collapse_subtree(&mut self, path: &str) {
        let paths: Vec<String> = if let Some(node) = self.find_node(path) {
            let mut stack = vec![node];
            let mut collected = Vec::new();
            while let Some(n) = stack.pop() {
                collected.push(n.path.clone());
                for c in &n.children {
                    if c.is_container() {
                        stack.push(c);
                    }
                }
            }
            collected
        } else {
            Vec::new()
        };
        for p in paths {
            self.expanded_nodes.remove(&p);
        }
        self.update_visible_rows();
    }

    pub fn ensure_visible(&mut self, path: &str) {
        if let Some((ancestors, true)) = self.find_with_ancestors(path) {
            for a in ancestors {
                self.expanded_nodes.insert(a);
            }
            self.update_visible_rows();
        }
    }

    pub fn clear(&mut self) {
        self.raw_text.clear();
        self.json_value = None;
        self.root = None;
        self.selected_path = None;
        self.parse_error = None;
        self.clear_search();
        self.expanded_nodes.clear();
        self.expanded_leaf_nodes.clear();
        self.visible_tree_rows.clear();
        self.update_metrics();
        self.is_dirty = false;
    }

    // MARK: - Transformations

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

    // MARK: - Copy variants

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
        self.root.as_ref().and_then(|r| find_in(r.as_ref(), path))
    }

    pub fn find_with_ancestors(&self, path: &str) -> Option<(Vec<String>, bool)> {
        let mut ancestors = Vec::new();
        let found = collect_ancestors(self.root.as_ref()?.as_ref(), path, &mut ancestors);
        if found { Some((ancestors, true)) } else { None }
    }

    pub fn parent_of_selected(&self) -> Option<(String, String)> {
        let sel = self.selected_path.as_deref()?;
        let root = self.root.as_ref()?;
        let parent = find_parent(root.as_ref(), sel)?;
        Some((parent.path.clone(), parent.key.clone()))
    }

    pub fn properties_for_selected(&self) -> (Option<(String, String)>, Vec<PropertyRow>) {
        let sel = match &self.selected_path {
            Some(p) => p.clone(),
            None => return (None, Vec::new()),
        };
        let root = match &self.root {
            Some(r) => r,
            None => return (None, Vec::new()),
        };
        let node = match find_in(root.as_ref(), &sel) {
            Some(n) => n,
            None => return (None, Vec::new()),
        };

        let parent_info = find_parent(root.as_ref(), &sel).map(|p| (p.path.clone(), p.key.clone()));

        // Match macOS PropertyGridView: if container, show its children; if leaf, show siblings from parent
        let container: &Node = if node.is_container() {
            node
        } else {
            match find_parent(root.as_ref(), &sel) {
                Some(p) => p,
                None => {
                    return (
                        None,
                        vec![PropertyRow {
                            name: node.key.clone(),
                            value: node.value_string_short(),
                            typ: node.value.type_name().to_string(),
                            path: node.path.clone(),
                            is_container: false,
                        }],
                    );
                }
            }
        };

        if container.children.is_empty() {
            return (
                parent_info,
                vec![PropertyRow {
                    name: container.key.clone(),
                    value: container.value_string_short(),
                    typ: container.value.type_name().to_string(),
                    path: container.path.clone(),
                    is_container: container.is_container(),
                }],
            );
        }

        let rows = container
            .children
            .iter()
            .map(|c| PropertyRow {
                name: c.key.clone(),
                value: c.value_string_short(),
                typ: c.value.type_name().to_string(),
                path: c.path.clone(),
                is_container: c.is_container(),
            })
            .collect();

        (parent_info, rows)
    }

    pub fn navigate_to_parent(&mut self) {
        if let Some((parent_path, _)) = self.parent_of_selected() {
            self.selected_path = Some(parent_path.clone());
            self.ensure_visible(&parent_path);
        }
    }

    pub fn navigate_to_property(&mut self, path: &str) {
        self.selected_path = Some(path.to_string());
        self.ensure_visible(path);
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
        self.search_result_ids.clear();
        self.search_status.clear();
        self.last_executed_query.clear();
        self.current_search_index = 0;
        self.update_visible_rows();
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
            Some(r) => r.clone(),
            None => {
                self.search_status = "Phrase not found!".to_string();
                return;
            }
        };
        let matches = search_in(root.as_ref(), &query);
        self.search_results = matches;
        self.search_result_ids = self.search_results.iter().cloned().collect();

        if self.search_results.is_empty() {
            self.search_status = "Phrase not found!".to_string();
        } else {
            self.current_search_index = 0;
            let target = self.search_results[0].clone();
            self.selected_path = Some(target.clone());
            self.ensure_visible(&target);
            self.search_status = format!("1 of {} matches", self.search_results.len());
        }
        self.update_visible_rows();
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
        self.current_search_index = (self.current_search_index + 1) % self.search_results.len();
        let target = self.search_results[self.current_search_index].clone();
        self.selected_path = Some(target.clone());
        self.ensure_visible(&target);
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
                self.selected_path = Some(target.clone());
                self.ensure_visible(&target);
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
        self.selected_path = Some(target.clone());
        self.ensure_visible(&target);
        self.search_status = format!("{} of {} matches", self.current_search_index + 1, self.search_results.len());
    }

    // MARK: - File I/O

    pub fn load_file(&mut self, path: &std::path::Path, settings: &Settings) -> Result<(), String> {
        match std::fs::read(path) {
            Ok(data) => match String::from_utf8(data) {
                Ok(s) => {
                    self.raw_text = s;
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

fn haystack_contains(haystack: &str, needle_lower: &str, needle_is_ascii: bool) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if needle_is_ascii {
        if haystack.is_ascii() {
            let n = needle_lower.as_bytes();
            let h = haystack.as_bytes();
            if n.len() > h.len() {
                return false;
            }
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
        return haystack.to_lowercase().contains(needle_lower);
    }
    haystack.to_lowercase().contains(needle_lower)
}

fn node_matches_search(node: &Node, q: &str, q_ascii: bool) -> bool {
    if haystack_contains(&node.key, q, q_ascii) {
        return true;
    }
    if haystack_contains(&node.path, q, q_ascii) {
        return true;
    }
    match &node.value {
        JSONValue::Null => haystack_contains("null", q, q_ascii),
        JSONValue::Bool(b) => haystack_contains(if *b { "true" } else { "false" }, q, q_ascii),
        JSONValue::Number(_, raw) => haystack_contains(raw, q, q_ascii),
        JSONValue::Str(s) => haystack_contains(s, q, q_ascii),
        JSONValue::Array(items) => {
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
