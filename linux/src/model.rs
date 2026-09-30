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
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

pub const SAMPLE_JSON: &str = "{\n  \"title\": \"JSON Viewer macOS\",\n  \"version\": \"1.0.0\",\n  \"description\": \"Native Mac JSON Viewer & Formatter\",\n  \"active\": true,\n  \"rating\": 4.95,\n  \"nullProperty\": null,\n  \"author\": {\n    \"name\": \"Antigravity & Stack.hu\",\n    \"email\": \"local@mac.internal\"\n  },\n  \"features\": [\n    \"Hierarchical Tree View\",\n    \"Property Grid Inspection\",\n    \"2-Space Indented Formatting\",\n    \"Whitespace Minification\",\n    \"Remote URL Loading\",\n    \"Full Key & Value Search\"\n  ],\n  \"statistics\": {\n    \"downloads\": 12840,\n    \"stars\": 892\n  }\n}";

#[derive(Clone, Debug)]
pub struct Node {
    /// Stable, unique internal identifier. Matches `path` unless the document
    /// contains repeated object keys, where extra `#2`, `#3`, … suffixes are
    /// appended so every member keeps its own identity.
    pub id: String,
    pub key: String,
    pub value: JSONValue,
    /// JSON path shown to the user. Not unique when object keys repeat.
    pub path: String,
    pub children: Vec<Node>,
}

#[allow(dead_code)]
impl Node {
    pub fn id(&self) -> &str {
        &self.id
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
        let mut used_ids: HashSet<String> = HashSet::new();
        build_node(root_key, value, "$", "$", &mut used_ids)
    }
}

fn build_node(key: &str, value: &JSONValue, path: &str, id: &str, used_ids: &mut HashSet<String>) -> Node {
    match value {
        JSONValue::Object(pairs) => {
            let mut children = Vec::with_capacity(pairs.len());
            for p in pairs {
                let child_path = format!("{}.{}", path, p.key);
                let candidate = if id == path { child_path.clone() } else { format!("{}.{}", id, p.key) };
                let child_id = unique_id(candidate, used_ids);
                children.push(build_node(&p.key, &p.value, &child_path, &child_id, used_ids));
            }
            Node { id: id.to_string(), key: key.to_string(), value: value.clone(), path: path.to_string(), children }
        }
        JSONValue::Array(items) => {
            let mut children = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let child_path = format!("{}[{}]", path, i);
                let candidate = if id == path { child_path.clone() } else { format!("{}[{}]", id, i) };
                let child_id = unique_id(candidate, used_ids);
                children.push(build_node(&i.to_string(), item, &child_path, &child_id, used_ids));
            }
            Node { id: id.to_string(), key: key.to_string(), value: value.clone(), path: path.to_string(), children }
        }
        _ => Node {
            id: id.to_string(),
            key: key.to_string(),
            value: value.clone(),
            path: path.to_string(),
            children: Vec::new(),
        },
    }
}

fn unique_id(candidate: String, used_ids: &mut HashSet<String>) -> String {
    if used_ids.insert(candidate.clone()) {
        return candidate;
    }
    let mut occurrence = 2usize;
    loop {
        let deduped = format!("{}#{}", candidate, occurrence);
        if used_ids.insert(deduped.clone()) {
            return deduped;
        }
        occurrence += 1;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyRow {
    pub name: String,
    pub value: String,
    pub typ: String,
    pub id: String,
    pub path: String,
    pub is_container: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParentInfo {
    pub id: String,
    pub path: String,
    pub key: String,
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
    pub id: String,
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
    /// Unique node ID of the selection (see [`Node::id`]).
    pub selected_id: Option<String>,
    pub parse_error: Option<JSONParseError>,
    pub error_message: String,
    pub search_query: String,
    /// Node IDs of the matches, in document order.
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
    /// Incremented after a node has been expanded and selected for tree
    /// navigation so the view can scroll once the rebuilt rows are available.
    pub tree_navigation_request: usize,
    pub requested_scroll_id: Option<String>,
    /// Incremented when clearing search should return the tree viewport to its root.
    pub tree_scroll_to_top_request: usize,
    /// Incremented on every successful parse; results computed against an older
    /// tree are discarded instead of pointing at rows that no longer exist.
    pub parse_generation: u64,
    /// True while a background search is still running.
    pub search_in_flight: bool,
    /// `1` / `-1` when the user asked for the next / previous match before the
    /// background search delivered its results.
    pub pending_search_advance: i8,
    search_generation: u64,
    search_cancel: Arc<AtomicBool>,
    search_tx: Sender<SearchCompletion>,
    search_rx: Receiver<SearchCompletion>,
}

/// Result handed back by the background search worker.
struct SearchCompletion {
    generation: u64,
    parse_generation: u64,
    query: String,
    matches: Vec<String>,
    reverse: bool,
}

impl DocumentModel {
    pub fn new(settings: &Settings) -> Self {
        let (search_tx, search_rx) = mpsc::channel();
        let mut m = Self {
            raw_text: SAMPLE_JSON.to_string(),
            json_value: None,
            root: None,
            selected_id: None,
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
            tree_navigation_request: 0,
            requested_scroll_id: None,
            tree_scroll_to_top_request: 0,
            parse_generation: 0,
            search_in_flight: false,
            pending_search_advance: 0,
            search_generation: 0,
            search_cancel: Arc::new(AtomicBool::new(false)),
            search_tx,
            search_rx,
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
        // Keep the current navigation state and restore the parts that still
        // exist after the rebuild (Split mode re-parses on every keystroke).
        let previous_selection_id = self.selected_id.clone();
        let previous_selection_path = self.selected_node().map(|n| n.path.clone());
        let previous_expanded = self.expanded_nodes.clone();
        let previous_expanded_leaf = self.expanded_leaf_nodes.clone();
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
        let root_id = root.id.clone();

        // Index the fresh tree so navigation state can be restored by ID, and
        // fall back to the JSON path when a repeated key changed its suffix.
        let mut nodes_by_id: HashSet<String> = HashSet::new();
        let mut nodes_by_path: HashMap<String, String> = HashMap::new();
        let mut container_ids: HashSet<String> = HashSet::new();
        let mut leaf_ids: HashSet<String> = HashSet::new();
        {
            let mut stack = vec![&root];
            while let Some(node) = stack.pop() {
                nodes_by_id.insert(node.id.clone());
                nodes_by_path.entry(node.path.clone()).or_insert_with(|| node.id.clone());
                if node.is_container() {
                    container_ids.insert(node.id.clone());
                } else {
                    leaf_ids.insert(node.id.clone());
                }
                for child in &node.children {
                    stack.push(child);
                }
            }
        }

        // Restore the selection when its node still exists, else fall back to root.
        let restored_selection = previous_selection_id
            .filter(|id| nodes_by_id.contains(id))
            .or_else(|| {
                previous_selection_path
                    .and_then(|p| nodes_by_path.get(&p).cloned())
            })
            .unwrap_or_else(|| root_id.clone());

        self.json_value = Some(parsed);
        self.root = Some(Arc::new(root));
        self.parse_error = None;
        self.is_dirty = false;
        self.parse_generation += 1;

        self.selected_id = Some(restored_selection);

        // Collapse branches that disappeared instead of keeping stale IDs.
        self.expanded_nodes = previous_expanded
            .into_iter()
            .filter(|id| container_ids.contains(id))
            .collect();
        self.expanded_leaf_nodes = previous_expanded_leaf
            .into_iter()
            .filter(|id| leaf_ids.contains(id))
            .collect();
        // macOS parity: the root container always stays expanded.
        self.expanded_nodes.insert(root_id);
        self.rebuild_search_results();
        self.update_visible_rows();
        true
    }

    pub fn update_visible_rows(&mut self) {
        self.visible_tree_rows.clear();
        let Some(root) = &self.root else { return };

        let mut stack: Vec<(&Node, usize)> = vec![(root.as_ref(), 0)];
        while let Some((node, depth)) = stack.pop() {
            let is_container = node.is_container();
            let is_expanded = self.expanded_nodes.contains(&node.id);
            let is_leaf_expanded = self.expanded_leaf_nodes.contains(&node.id);
            let is_match = self.search_result_ids.contains(&node.id);
            let is_long = node.is_long_text();
            let (full_str, char_count) = match &node.value {
                JSONValue::Str(s) => (Some(s.clone()), s.chars().count()),
                _ => (None, 0),
            };

            let row = FlatTreeRow {
                id: node.id.clone(),
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

    pub fn toggle_expand(&mut self, id: &str) {
        if self.expanded_nodes.contains(id) {
            self.expanded_nodes.remove(id);
        } else {
            self.expanded_nodes.insert(id.to_string());
        }
        self.update_visible_rows();
    }

    pub fn toggle_expand_leaf(&mut self, id: &str) {
        if self.expanded_leaf_nodes.contains(id) {
            self.expanded_leaf_nodes.remove(id);
        } else {
            self.expanded_leaf_nodes.insert(id.to_string());
        }
        self.update_visible_rows();
    }

    pub fn expand_all(&mut self) {
        if let Some(root) = &self.root {
            let mut stack = vec![root.as_ref()];
            let mut count = 0usize;
            while let Some(n) = stack.pop() {
                if n.is_container() {
                    self.expanded_nodes.insert(n.id.clone());
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
            self.expanded_nodes.insert(root.id.clone());
        }
        self.update_visible_rows();
    }

    pub fn expand_subtree(&mut self, id: &str) {
        let paths: Vec<String> = if let Some(node) = self.find_node(id) {
            let mut stack = vec![node];
            let mut collected = Vec::new();
            while let Some(n) = stack.pop() {
                if n.is_container() {
                    collected.push(n.id.clone());
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

    pub fn collapse_subtree(&mut self, id: &str) {
        let paths: Vec<String> = if let Some(node) = self.find_node(id) {
            let mut stack = vec![node];
            let mut collected = Vec::new();
            while let Some(n) = stack.pop() {
                collected.push(n.id.clone());
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

    /// Expand the node's ancestors (and the node itself when it is a container)
    /// so the row exists in `visible_tree_rows`, then signal the view to scroll.
    pub fn ensure_visible(&mut self, id: &str) {
        let mut did_expand = false;
        if let Some((ancestors, _)) = self.find_with_ancestors(id) {
            for a in ancestors {
                if self.expanded_nodes.insert(a) {
                    did_expand = true;
                }
            }
            if let Some(node) = self.find_node(id) {
                if node.is_container() && self.expanded_nodes.insert(id.to_string()) {
                    did_expand = true;
                }
            }
        } else if let Some(root) = &self.root {
            if root.id == id && self.expanded_nodes.insert(root.id.clone()) {
                did_expand = true;
            }
        }
        if did_expand {
            self.update_visible_rows();
        }
        // Signal after expanding ancestors and rebuilding rows so the view can
        // scroll once the virtualized row is part of the visible tree.
        self.tree_navigation_request += 1;
    }

    pub fn clear(&mut self) {
        self.raw_text.clear();
        self.json_value = None;
        self.root = None;
        self.selected_id = None;
        self.requested_scroll_id = None;
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

    /// Currently selected node, if the selection still resolves.
    pub fn selected_node(&self) -> Option<&Node> {
        let id = self.selected_id.as_deref()?;
        self.find_node(id)
    }

    /// JSON path of the selection, shown in the tree toolbar.
    pub fn selected_path(&self) -> Option<String> {
        self.selected_node().map(|n| n.path.clone())
    }

    pub fn find_node(&self, id: &str) -> Option<&Node> {
        self.root.as_ref().and_then(|r| find_by_id(r.as_ref(), id))
    }

    pub fn find_with_ancestors(&self, id: &str) -> Option<(Vec<String>, bool)> {
        let mut ancestors = Vec::new();
        let found = collect_ancestors(self.root.as_ref()?.as_ref(), id, &mut ancestors);
        if found {
            Some((ancestors, true))
        } else {
            None
        }
    }

    pub fn parent_of_selected(&self) -> Option<ParentInfo> {
        let sel = self.selected_id.as_deref()?;
        let root = self.root.as_ref()?;
        let parent = find_parent(root.as_ref(), sel)?;
        Some(ParentInfo { id: parent.id.clone(), path: parent.path.clone(), key: parent.key.clone() })
    }

    pub fn properties_for_selected(&self) -> (Option<ParentInfo>, Vec<PropertyRow>) {
        let sel = match &self.selected_id {
            Some(p) => p.clone(),
            None => return (None, Vec::new()),
        };
        let root = match &self.root {
            Some(r) => r,
            None => return (None, Vec::new()),
        };
        let node = match find_by_id(root.as_ref(), &sel) {
            Some(n) => n,
            None => return (None, Vec::new()),
        };

        let parent_info = find_parent(root.as_ref(), &sel)
            .map(|p| ParentInfo { id: p.id.clone(), path: p.path.clone(), key: p.key.clone() });

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
                            id: node.id.clone(),
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
                    id: container.id.clone(),
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
                id: c.id.clone(),
                path: c.path.clone(),
                is_container: c.is_container(),
            })
            .collect();

        (parent_info, rows)
    }

    pub fn navigate_to_parent(&mut self) {
        if let Some(parent) = self.parent_of_selected() {
            self.selected_id = Some(parent.id.clone());
            self.ensure_visible(&parent.id);
            self.requested_scroll_id = Some(parent.id);
        }
    }

    pub fn navigate_to_property(&mut self, id: &str) {
        self.select_and_reveal(id);
    }

    pub fn select_and_reveal(&mut self, id: &str) {
        self.selected_id = Some(id.to_string());
        self.ensure_visible(id);
        self.requested_scroll_id = Some(id.to_string());
    }

    /// Re-request a scroll to the current selection, e.g. when the tree
    /// becomes visible again after editing in another tab.
    pub fn request_scroll_to_selection(&mut self) {
        if let Some(id) = self.selected_id.clone() {
            self.requested_scroll_id = Some(id);
            self.tree_navigation_request += 1;
        }
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
        self.cancel_search();
        self.search_query.clear();
        self.search_results.clear();
        self.search_result_ids.clear();
        self.search_status.clear();
        self.last_executed_query.clear();
        self.requested_scroll_id = None;
        self.current_search_index = 0;
        // The query is empty now, so the tree viewport belongs back at the root.
        self.tree_scroll_to_top_request += 1;
        self.update_visible_rows();
    }

    /// Cancel the running search, if any. Outstanding completions are ignored
    /// because the generation moved on.
    fn cancel_search(&mut self) {
        self.search_generation += 1;
        self.search_cancel.store(true, Ordering::Relaxed);
        self.search_in_flight = false;
        self.pending_search_advance = 0;
    }

    /// Drop matches whose node IDs no longer exist after a re-parse and keep
    /// the highlight set in sync.
    fn rebuild_search_results(&mut self) {
        if self.search_results.is_empty() {
            self.search_result_ids.clear();
            return;
        }
        let root = self.root.as_ref();
        let mut kept: Vec<String> = Vec::with_capacity(self.search_results.len());
        let mut seen: HashSet<String> = HashSet::new();
        for id in self.search_results.drain(..) {
            let exists = root
                .as_ref()
                .map(|r| !seen.contains(&id) && find_by_id_exists(r.as_ref(), &id))
                .unwrap_or(false);
            if exists && seen.insert(id.clone()) {
                kept.push(id);
            }
        }
        self.search_results = kept;
        if self.current_search_index >= self.search_results.len() {
            self.current_search_index = self.search_results.len().saturating_sub(1);
        }
        self.search_result_ids = self.search_results.iter().cloned().collect();
    }

    /// Select the match at `index`, expanding its ancestors and asking the view
    /// to scroll to it.
    fn select_match(&mut self, index: usize) {
        let target = match self.search_results.get(index) {
            Some(t) => t.clone(),
            None => return,
        };
        self.current_search_index = index;
        self.select_and_reveal(&target);
        self.search_status = format!("{} of {} matches", index + 1, self.search_results.len());
    }

    /// Start a search on a cancellable background task so clearing or editing
    /// the query stays responsive on large documents.
    pub fn search_start(&mut self, settings: &Settings, reverse: bool) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.clear_search();
            return;
        }

        self.cancel_search();
        let generation = self.search_generation;
        self.last_executed_query = query.clone();

        if self.root.is_none() && !self.parse_and_build_tree(true, settings) {
            self.search_status = "Phrase not found!".to_string();
            self.requested_scroll_id = None;
            return;
        }
        let root = match &self.root {
            Some(r) => r.clone(),
            None => {
                self.search_status = "Phrase not found!".to_string();
                self.requested_scroll_id = None;
                return;
            }
        };

        let parse_generation = self.parse_generation;
        self.search_results.clear();
        self.search_result_ids.clear();
        self.current_search_index = 0;
        self.search_status = "Searching\u{2026}".to_string();
        self.search_in_flight = true;

        self.search_cancel.store(false, Ordering::Relaxed);
        let cancel = self.search_cancel.clone();
        let worker_query = query.clone();
        let tx = self.search_tx.clone();
        std::thread::spawn(move || {
            let matches = search_nodes(root.as_ref(), &worker_query, &cancel);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let _ = tx.send(SearchCompletion { generation, parse_generation, query: worker_query, matches, reverse });
        });
    }

    /// Apply finished background searches. Results from canceled, edited, or
    /// outdated queries are discarded.
    pub fn poll_search_results(&mut self) {
        let mut latest: Option<SearchCompletion> = None;
        while let Ok(completion) = self.search_rx.try_recv() {
            latest = Some(completion);
        }
        let Some(completion) = latest else { return };
        if completion.generation != self.search_generation
            || completion.parse_generation != self.parse_generation
            || completion.query != self.search_query.trim()
        {
            return;
        }
        self.search_in_flight = false;

        let advance = self.pending_search_advance;
        self.pending_search_advance = 0;
        self.search_results = completion.matches;
        self.search_result_ids = self.search_results.iter().cloned().collect();

        if self.search_results.is_empty() {
            self.current_search_index = 0;
            self.search_status = "Phrase not found!".to_string();
            self.requested_scroll_id = None;
            self.update_visible_rows();
            return;
        }

        let last = self.search_results.len() - 1;
        let index = if advance < 0 || completion.reverse {
            last
        } else if advance > 0 {
            (last).min(1)
        } else {
            0
        };
        self.select_match(index);
        self.update_visible_rows();
    }

    pub fn search_next(&mut self, settings: &Settings) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        if query != self.last_executed_query || self.search_results.is_empty() {
            if self.search_in_flight {
                // Remember the intent so the finished search advances once.
                self.pending_search_advance = 1;
                return;
            }
            self.search_start(settings, false);
            return;
        }
        let next = (self.current_search_index + 1) % self.search_results.len();
        self.select_match(next);
        self.update_visible_rows();
    }

    pub fn search_previous(&mut self, settings: &Settings) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            return;
        }
        if query != self.last_executed_query || self.search_results.is_empty() {
            if self.search_in_flight {
                self.pending_search_advance = -1;
                return;
            }
            self.search_start(settings, true);
            return;
        }
        let previous = if self.current_search_index == 0 {
            self.search_results.len() - 1
        } else {
            self.current_search_index - 1
        };
        self.select_match(previous);
        self.update_visible_rows();
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

fn find_by_id<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
    if node.id == id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(f) = find_by_id(child, id) {
            return Some(f);
        }
    }
    None
}

/// Iterative variant used when only existence matters.
fn find_by_id_exists(node: &Node, id: &str) -> bool {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if n.id == id {
            return true;
        }
        for c in &n.children {
            stack.push(c);
        }
    }
    false
}

fn find_parent<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
    for child in &node.children {
        if child.id == id {
            return Some(node);
        }
        if let Some(f) = find_parent(child, id) {
            return Some(f);
        }
    }
    None
}

fn collect_ancestors(node: &Node, target: &str, stack: &mut Vec<String>) -> bool {
    if node.id == target {
        return true;
    }
    for child in &node.children {
        stack.push(node.id.clone());
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

/// Collect matching node IDs in document order, stopping early when the
/// background task is canceled.
fn search_nodes(node: &Node, query: &str, cancel: &AtomicBool) -> Vec<String> {
    let q = query.to_lowercase();
    let q_ascii = q.is_ascii();
    let mut out = Vec::new();
    // One accumulator plus an explicit stack: descendant matches are appended
    // once instead of being copied out of every recursion level.
    let mut stack: Vec<&Node> = vec![node];
    while let Some(n) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if node_matches_search(n, &q, q_ascii) {
            out.push(n.id.clone());
        }
        for child in n.children.iter().rev() {
            stack.push(child);
        }
    }
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

