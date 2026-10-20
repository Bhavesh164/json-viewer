//! Document model — Rust port of `Sources/JSONViewerCore/JSONDocumentModel.swift`
//! plus `JSONNode.swift` tree helpers. UI-agnostic: the Win32 layer in
//! `app.rs` owns a `DocumentModel` and mirrors its state into native controls.
//!
//! Windows parity notes:
//! - Tree nodes carry a unique internal `id` in addition to their JSON `path`,
//!   so repeated object keys (`{"count": 3, "count": 23423}`) are represented
//!   exactly like on macOS/Linux (last value wins, position of first kept).
//! - Selection is tracked by that id, which lets Split mode preserve the
//!   selected node and expanded branches across re-parses.
//! - Search runs on a cancellable background worker over a shared `Arc<Node>`
//!   tree, so editing/clearing the query never freezes the UI on large files.

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
    /// contains repeated object keys, where a `#2`, `#3`, … suffix is appended
    /// so every member keeps its own identity.
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

    /// Short display form for the property grid (macOS/Linux parity: quoted
    /// short strings, ellipsized long text, type-appropriate summaries).
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

    /// True if a string exceeds 45 chars or contains multiple lines (macOS
    /// expandable-text threshold).
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

    pub fn type_name(&self) -> &'static str {
        self.value.type_name()
    }

    pub fn build_tree(value: &JSONValue, root_key: &str) -> Node {
        let mut used_ids: HashSet<String> = HashSet::new();
        used_ids.insert("$".to_string());
        build_node(root_key, value, "$", "$", &mut used_ids)
    }
}

fn build_node(key: &str, value: &JSONValue, path: &str, id: &str, used_ids: &mut HashSet<String>) -> Node {
    match value {
        JSONValue::Object(pairs) => {
            // An object may legally repeat a key, and the editor lets you type
            // one. The tree is a *view*, so it shows the value a consumer of
            // the document actually sees: the last occurrence wins, exactly as
            // `JSON.parse` does, at the position of the first occurrence.
            // The text itself is left alone, so nothing the user wrote is
            // silently rewritten.
            let mut effective: Vec<&crate::json::JSONProperty> = Vec::with_capacity(pairs.len());
            for (i, p) in pairs.iter().enumerate() {
                match effective.iter().position(|q| q.key == p.key) {
                    Some(at) => effective[at] = &pairs[i],
                    None => effective.push(p),
                }
            }

            // Like macOS, the node itself exposes the resolved object (last
            // duplicate wins), so its `{N}` badge, property-grid summary and
            // Copy Subtree all describe what a JSON consumer would see. The
            // original text keeps every pair for Format / Save round-trips.
            let resolved_value = JSONValue::Object(
                effective
                    .iter()
                    .map(|p| crate::json::JSONProperty::new(p.key.clone(), p.value.clone()))
                    .collect(),
            );
            let mut children = Vec::with_capacity(effective.len());
            for p in effective {
                let child_path = format!("{}.{}", path, p.key);
                let candidate = if id == path { child_path.clone() } else { format!("{}.{}", id, p.key) };
                let child_id = unique_id(candidate, used_ids);
                children.push(build_node(&p.key, &p.value, &child_path, &child_id, used_ids));
            }
            Node { id: id.to_string(), key: key.to_string(), value: resolved_value, path: path.to_string(), children }
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
    pub id: String,
    pub label: String,
    pub depth: usize,
    pub is_container: bool,
    pub type_name: &'static str,
}

/// Flatten the tree in pre-order (iterative — safe for any depth).
pub fn flatten(root: &Node) -> Vec<FlatRow> {
    let mut out = Vec::new();
    let mut stack: Vec<(&Node, usize)> = vec![(root, 0)];
    while let Some((node, depth)) = stack.pop() {
        out.push(FlatRow {
            id: node.id.clone(),
            label: node.tree_label(),
            depth,
            is_container: node.is_container(),
            type_name: node.value.type_name(),
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
    /// Incremented on every successful parse; results computed against an
    /// older tree are discarded instead of pointing at rows that no longer
    /// exist.
    pub parse_generation: u64,
    /// True while a background search is still running.
    pub search_in_flight: bool,
    /// `1` / `-1` when the user asked for the next / previous match before
    /// the background search delivered its results.
    pending_search_advance: i8,
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
            parse_generation: 0,
            search_in_flight: false,
            pending_search_advance: 0,
            search_generation: 0,
            search_cancel: Arc::new(AtomicBool::new(false)),
            search_tx,
            search_rx,
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

    pub fn selected_node(&self) -> Option<&Node> {
        let root = self.root.as_ref()?;
        let id = self.selected_id.as_deref()?;
        find_by_id(root.as_ref(), id)
    }

    pub fn selected_path(&self) -> Option<String> {
        self.selected_node().map(|n| n.path.clone())
    }

    pub fn parse_and_build_tree(&mut self, silent: bool, settings: &Settings) -> bool {
        // Keep the current navigation state and restore the parts that still
        // exist after the rebuild (Split mode re-parses on every edit).
        let previous_selection_id = self.selected_id.clone();
        let previous_selection_path = self.selected_node().map(|n| n.path.clone());
        let trimmed = self.raw_text.trim();
        if trimmed.is_empty() {
            // An empty document cannot correspond to any tree, so there is
            // nothing worth keeping stale: drop the last good tree and any
            // search results that pointed into it.
            self.discard_tree();
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
        {
            let mut stack = vec![&root];
            while let Some(node) = stack.pop() {
                nodes_by_id.insert(node.id.clone());
                nodes_by_path.entry(node.path.clone()).or_insert_with(|| node.id.clone());
                for child in &node.children {
                    stack.push(child);
                }
            }
        }
        let restored_selection = previous_selection_id
            .filter(|id| nodes_by_id.contains(id))
            .or_else(|| previous_selection_path.and_then(|p| nodes_by_path.get(&p).cloned()))
            .unwrap_or_else(|| root_id.clone());

        // A new tree invalidates results from any in-flight search.
        self.cancel_search();
        self.json_value = Some(parsed);
        self.root = Some(Arc::new(root));
        self.parse_error = None;
        self.error_message.clear();
        self.is_dirty = false;
        self.parse_generation += 1;
        self.selected_id = Some(restored_selection);
        self.rebuild_search_results();
        true
    }

    /// Drop the parsed document and everything derived from it, leaving the
    /// text itself untouched.
    pub fn discard_tree(&mut self) {
        self.json_value = None;
        self.root = None;
        self.selected_id = None;
        self.parse_error = None;
        self.error_message.clear();
        self.cancel_search();
        self.clear_search();
        self.parse_generation += 1;
    }

    pub fn clear(&mut self) {
        self.raw_text.clear();
        self.discard_tree();
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

    pub fn find_node(&self, id: &str) -> Option<&Node> {
        let root = self.root.as_ref()?;
        find_by_id(root.as_ref(), id)
    }

    pub fn find_with_ancestors(&self, id: &str) -> Option<(Vec<String>, bool)> {
        // Returns (ancestor IDs from root to parent, found).
        let mut ancestors = Vec::new();
        let found = collect_ancestors(self.root.as_ref()?.as_ref(), id, &mut ancestors);
        if found { Some((ancestors, true)) } else { None }
    }

    pub fn parent_of_selected(&self) -> Option<ParentInfo> {
        let sel = self.selected_id.as_deref()?;
        let root = self.root.as_ref()?;
        let parent = find_parent(root.as_ref(), sel)?;
        Some(ParentInfo { id: parent.id.clone(), path: parent.path.clone(), key: parent.key.clone() })
    }

    pub fn properties_for_selected_limited(&self, limit: usize) -> (Option<ParentInfo>, usize, Vec<PropertyRow>) {
        let sel = match &self.selected_id {
            Some(p) => p.clone(),
            None => return (None, 0, Vec::new()),
        };
        let root = match &self.root {
            Some(r) => r,
            None => return (None, 0, Vec::new()),
        };
        let node = match find_by_id(root.as_ref(), &sel) {
            Some(n) => n,
            None => return (None, 0, Vec::new()),
        };

        let parent_info = find_parent(root.as_ref(), &sel)
            .map(|p| ParentInfo { id: p.id.clone(), path: p.path.clone(), key: p.key.clone() });

        // Match macOS: if leaf, show parent's properties (siblings).
        let container: &Node = if node.is_container() {
            node
        } else {
            match find_parent(root.as_ref(), &sel) {
                Some(p) => p,
                None => {
                    return (
                        None,
                        usize::from(!node.is_container()),
                        vec![PropertyRow {
                            name: node.key.clone(),
                            value: node.value_string_short(),
                            typ: node.value.type_name().to_string(),
                            id: node.id.clone(),
                            path: node.path.clone(),
                            is_container: node.is_container(),
                        }],
                    );
                }
            }
        };

        if container.children.is_empty() {
            return (
                parent_info,
                usize::from(!container.is_container()),
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

        // Cap while building: a root array can have hundreds of thousands of
        // children, and allocating a display string for every one of them
        // just to throw them away would freeze the UI.
        let total = container.children.len();
        let take = total.min(limit);
        let rows = container
            .children
            .iter()
            .take(take)
            .map(|c| PropertyRow {
                name: c.key.clone(),
                value: c.value_string_short(),
                typ: c.value.type_name().to_string(),
                id: c.id.clone(),
                path: c.path.clone(),
                is_container: c.is_container(),
            })
            .collect();

        (parent_info, total, rows)
    }

    pub fn set_selected(&mut self, id: impl Into<String>) {
        self.selected_id = Some(id.into());
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
        self.current_search_index = 0;
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

    /// Select the match at `index` and update the "N of M matches" status.
    fn select_match(&mut self, index: usize) {
        let target = match self.search_results.get(index) {
            Some(t) => t.clone(),
            None => return,
        };
        self.current_search_index = index;
        self.selected_id = Some(target);
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
            return;
        }
        let root = match &self.root {
            Some(r) => r.clone(),
            None => {
                self.search_status = "Phrase not found!".to_string();
                return;
            }
        };

        let parse_generation = self.parse_generation;
        self.search_results.clear();
        self.search_result_ids.clear();
        self.current_search_index = 0;
        self.search_status = "Searching…".to_string();
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

    /// Apply a finished background search. Returns true when a completion was
    /// applied so the UI can reveal/scroll to the newly selected match.
    /// Results from canceled, edited, or outdated queries are discarded.
    pub fn poll_search_results(&mut self) -> bool {
        let mut latest: Option<SearchCompletion> = None;
        while let Ok(completion) = self.search_rx.try_recv() {
            latest = Some(completion);
        }
        let Some(completion) = latest else { return false };
        if completion.generation != self.search_generation
            || completion.parse_generation != self.parse_generation
        {
            return false;
        }
        self.search_in_flight = false;
        // The query was edited while the worker ran: the user is about to
        // press Go/Enter again, so discard the stale result instead of
        // highlighting matches for text that is no longer in the box.
        if completion.query != self.search_query.trim() {
            self.search_status.clear();
            return false;
        }

        let advance = self.pending_search_advance;
        self.pending_search_advance = 0;
        self.search_results = completion.matches;
        self.search_result_ids = self.search_results.iter().cloned().collect();

        if self.search_results.is_empty() {
            self.current_search_index = 0;
            self.search_status = "Phrase not found!".to_string();
            return true;
        }

        let last = self.search_results.len() - 1;
        let index = if advance < 0 || completion.reverse {
            last
        } else if advance > 0 {
            last.min(1)
        } else {
            0
        };
        self.select_match(index);
        true
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> DocumentModel {
        let settings = Settings::default();
        let mut model = DocumentModel::new(&settings);
        model.raw_text = text.to_string();
        assert!(model.parse_and_build_tree(true, &settings), "fixture must parse");
        model
    }

    #[test]
    fn repeated_object_keys_resolve_to_last_value_at_first_position() {
        let model = parsed(r#"{"a":1,"count":3,"b":2,"count":23423,"c":3}"#);
        let root = model.root.as_ref().unwrap();
        let keys: Vec<&str> = root.children.iter().map(|n| n.key.as_str()).collect();
        assert_eq!(keys, ["a", "count", "b", "c"]);
        let count = root.children.iter().find(|n| n.key == "count").unwrap();
        assert_eq!(count.value_string(), "23423");
        // The tree node's own value is the resolved object, so its badge and
        // copied subtree describe four unique properties, not the raw five
        // pairs.
        assert_eq!(root.tree_label(), "JSON {4}");
        assert!(!root.formatted_value_string().contains("\"count\": 3"));
        // Repeated keys are deduplicated per object, not per document.
        let array = parsed(r#"[{"k":1},{"k":2}]"#);
        let items = &array.root.as_ref().unwrap().children;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].children[0].key, "k");
        assert_eq!(items[1].children[0].key, "k");
    }

    #[test]
    fn node_ids_are_unique_and_selection_tracks_id() {
        let mut model = parsed(r#"{"outer":{"inner":[1,2,3]},"z":0}"#);
        let root = model.root.as_ref().unwrap();
        let mut ids = HashSet::new();
        let mut stack = vec![root.as_ref()];
        while let Some(n) = stack.pop() {
            assert!(ids.insert(n.id.clone()), "duplicate id {}", n.id);
            for c in &n.children {
                stack.push(c);
            }
        }
        let inner = model
            .find_node("$.outer.inner")
            .expect("array node exists by id");
        assert_eq!(inner.path, "$.outer.inner");
        model.set_selected("$.outer.inner[1]");
        assert_eq!(model.selected_path().as_deref(), Some("$.outer.inner[1]"));
        assert_eq!(model.selected_node().unwrap().value_string(), "2");
    }

    #[test]
    fn leaf_property_grid_shows_parents_siblings() {
        let mut model = parsed(r#"{"a":1,"b":2}"#);
        model.set_selected("$.a");
        let (parent, total, rows) = model.properties_for_selected_limited(GRID_TEST_CAP);
        assert_eq!(parent.as_ref().unwrap().path, "$");
        assert_eq!(total, 2);
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    }

    const GRID_TEST_CAP: usize = 10_000;

    #[test]
    fn trailing_content_is_not_mistaken_for_valid_json() {
        let settings = Settings::default();
        let mut model = DocumentModel::new(&settings);
        model.discard_tree();
        model.raw_text = r#"{"a":1}x"#.to_string();
        assert!(!model.parse_and_build_tree(true, &settings));
        assert!(model.parse_error.is_some());
        assert!(model.root.is_none());
    }

    #[test]
    fn background_search_finds_values_and_can_be_cancelled() {
        let settings = Settings::default();
        let mut model = parsed(r#"{"alpha":"needle","beta":"haystack","gamma":["needle",4]}"#);
        model.search_query = "needle".to_string();
        model.search_start(&settings, false);
        let mut applied = false;
        for _ in 0..200 {
            if model.poll_search_results() {
                applied = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(applied, "search worker did not finish");
        assert_eq!(model.search_results.len(), 2);
        assert!(model.search_results.contains(&"$.alpha".to_string()));
        assert!(model.search_results.contains(&"$.gamma[0]".to_string()));
        assert_eq!(model.search_status, "1 of 2 matches");

        model.search_query.clear();
        model.search_start(&settings, false);
        assert!(model.search_results.is_empty());
        assert!(!model.search_in_flight);
    }
}
