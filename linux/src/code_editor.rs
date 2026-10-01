//! Virtualized code editor — the egui counterpart of the AppKit
//! `NSTextStorage` + `NSTextView` pair used by the macOS app
//! (`Sources/JSONViewer/Views/TextEditorView.swift`).
//!
//! # Why this exists
//!
//! The macOS editor stays usable on multi-megabyte JSON because
//! `NativeCodeEditor` sets `layoutManager.allowsNonContiguousLayout = true`, so
//! `NSTextView` only lays out the lines that intersect the visible rect. egui
//! has no equivalent mode: `TextEdit::multiline` re-shapes and re-wraps the
//! whole buffer on every frame. Measured on this machine, a 7.7 MB / 400k line
//! document costs **~543 ms per frame** in the Text tab, which makes the app
//! unusable (see `benches/text_tab.rs`).
//!
//! # The same two-part design as AppKit
//!
//! * [`TextBuffer`] plays the `NSTextStorage` role: it owns the flat text plus a
//!   line index (the byte offset of every line start). The index is built once
//!   per text change and is *never* walked per frame beyond the visible rows.
//! * [`CodeEditor`] plays the `NSTextView` role: it owns the cursor, the
//!   selection, scrolling and undo, and every frame it shapes only the handful
//!   of lines that intersect the viewport.
//!
//! Per-frame cost is therefore O(visible lines), independent of document size.

use std::borrow::Cow;
use std::ops::Range;

use egui::{Align2, Event, FontId, Key, Pos2, Rect, Sense, Vec2};

/// Columns per tab stop when a line contains tab characters.
const TAB_COLUMNS: usize = 4;

/// Horizontal padding inside the editor, in points.
const PAD_X: f32 = 6.0;

/// Seconds for one half of the cursor blink cycle.
const BLINK_PERIOD: f32 = 0.5;

/// Window in which a second click on the same line counts as a triple click.
const TRIPLE_CLICK_SECONDS: f64 = 0.4;

/// Number of columns a character occupies in a monospaced grid. CJK and emoji
/// are double width, matching terminals and `NSTextView`.
fn char_cols(c: char) -> usize {
    let cp = c as u32;
    let wide = matches!(cp,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD);
    if wide {
        2
    } else {
        1
    }
}

/// Visual width of `s` in monospaced columns, expanding tabs to tab stops.
fn visual_cols(s: &str) -> usize {
    let mut col = 0;
    for c in s.chars() {
        if c == '\t' {
            col += TAB_COLUMNS - (col % TAB_COLUMNS);
        } else {
            col += char_cols(c);
        }
    }
    col
}

/// A text position: a logical line plus a display column inside that line.
///
/// Columns count display cells rather than bytes so they stay correct for UTF-8
/// input and for lines that contain tabs or wide characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
}

impl Cursor {
    pub const START: Self = Self { line: 0, col: 0 };
}

/// The `NSTextStorage` role: flat text plus a line index.
///
/// The index maps logical lines to byte offsets, so the editor can jump to any
/// line without scanning the document and can slice a line in O(1).
#[derive(Clone, Debug)]
pub struct TextBuffer {
    text: String,
    /// Byte offset of the first byte of each line. Always at least one entry
    /// and `line_starts[0] == 0`.
    line_starts: Vec<usize>,
    /// Widest line in display columns, and the line that holds it.
    max_cols: usize,
    max_line: usize,
    /// True when the text is pure ASCII *and* free of tabs. Then byte offsets,
    /// character indices and display columns are all the same number, which
    /// lets the hot paths skip char iteration entirely.
    simple: bool,
    /// Bumped on every mutation so callers can drop text-keyed caches.
    version: u64,
}

struct Index {
    line_starts: Vec<usize>,
    max_cols: usize,
    max_line: usize,
    simple: bool,
}

/// Build a line index plus width statistics for `text`.
fn scan(text: &str) -> Index {
    let simple = text.is_ascii() && !text.as_bytes().contains(&b'\t');

    // `split` is memchr-backed, so this stays a fast linear pass on 8 MB.
    let mut line_starts = Vec::with_capacity(text.bytes().filter(|&b| b == b'\n').count() + 1);
    let mut offset = 0usize;
    for part in text.split('\n') {
        line_starts.push(offset);
        offset += part.len() + 1;
    }

    let mut max_cols = 0usize;
    let mut max_line = 0usize;
    for line in 0..line_starts.len() {
        let cols = line_cols_in(text, &line_starts, line, simple);
        if cols > max_cols {
            max_cols = cols;
            max_line = line;
        }
    }
    Index {
        line_starts,
        max_cols,
        max_line,
        simple,
    }
}

/// Width in display columns of line `line`, given precomputed line starts.
fn line_cols_in(text: &str, line_starts: &[usize], line: usize, simple: bool) -> usize {
    let start = line_starts[line];
    let end = match line_starts.get(line + 1) {
        // Drop the '\n' that `split` consumed.
        Some(next) => next - 1,
        None => text.len(),
    };
    if simple {
        end - start
    } else {
        visual_cols(&text[start..end])
    }
}

impl TextBuffer {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let idx = scan(&text);
        Self {
            text,
            line_starts: idx.line_starts,
            max_cols: idx.max_cols,
            max_line: idx.max_line,
            simple: idx.simple,
            version: 1,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    /// Number of logical lines. An empty document still has one (empty) line.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Widest line in display columns; drives the horizontal scroll extent.
    pub fn max_cols(&self) -> usize {
        self.max_cols
    }

    /// True when display columns equal byte offsets (ASCII, no tabs).
    pub fn is_simple(&self) -> bool {
        self.simple
    }

    /// Byte range of `line`, excluding the trailing newline.
    fn line_range(&self, line: usize) -> Range<usize> {
        let start = self.line_starts[line];
        let end = match self.line_starts.get(line + 1) {
            Some(next) => next - 1,
            None => self.text.len(),
        };
        start..end
    }

    pub fn line_str(&self, line: usize) -> &str {
        let r = self.line_range(line);
        &self.text[r]
    }

    /// Display width of `line` in columns.
    pub fn line_cols(&self, line: usize) -> usize {
        if self.simple {
            let r = self.line_range(line);
            r.end - r.start
        } else {
            visual_cols(self.line_str(line))
        }
    }

    /// Clamp `c` to a position that exists in the document.
    pub fn clamp(&self, c: Cursor) -> Cursor {
        let line = c.line.min(self.line_count() - 1);
        Cursor {
            line,
            col: c.col.min(self.line_cols(line)),
        }
    }

    /// Absolute byte offset of `c`.
    pub fn byte_of(&self, c: Cursor) -> usize {
        let c = self.clamp(c);
        let r = self.line_range(c.line);
        if self.simple {
            return r.start + c.col;
        }
        r.start + self.byte_in_line_of_col(c.line, c.col)
    }

    /// Line-relative byte offset of the character at display column `col`.
    fn byte_in_line_of_col(&self, line: usize, col: usize) -> usize {
        let s = self.line_str(line);
        if self.simple {
            return col.min(s.len());
        }
        let mut at = 0usize;
        for (i, c) in s.char_indices() {
            if at >= col {
                return i;
            }
            at += if c == '\t' {
                TAB_COLUMNS - (at % TAB_COLUMNS)
            } else {
                char_cols(c)
            };
        }
        s.len()
    }

    /// Display column of a line-relative byte offset.
    fn col_in_line_of_byte(&self, line: usize, byte: usize) -> usize {
        let s = self.line_str(line);
        let byte = byte.min(s.len());
        if self.simple {
            return byte;
        }
        visual_cols(&s[..floor_char_boundary(s, byte)])
    }

    /// Position of an absolute byte offset.
    pub fn cursor_of(&self, byte: usize) -> Cursor {
        let byte = byte.min(self.text.len());
        let line = self.line_starts.partition_point(|&s| s <= byte) - 1;
        let rel = byte - self.line_starts[line];
        let col = if self.simple {
            rel
        } else {
            self.col_in_line_of_byte(line, rel)
        };
        Cursor { line, col }
    }

    /// The part of `line` that falls inside columns `col_start..col_end`,
    /// together with the column the returned text starts at.
    ///
    /// Borrowed and allocation free in the common ASCII/no-tab case; otherwise
    /// tabs are expanded to spaces so painted columns line up with the cursor.
    fn visible_slice(&self, line: usize, col_start: usize, col_end: usize) -> (Cow<'_, str>, usize) {
        let s = self.line_str(line);
        if self.simple {
            let start = col_start.min(s.len());
            let end = col_end.clamp(start, s.len());
            return (Cow::Borrowed(&s[start..end]), start);
        }
        let from = self.byte_in_line_of_col(line, col_start);
        let to = self.byte_in_line_of_col(line, col_end).max(from);
        let mut out = String::with_capacity(to - from);
        let mut col = 0usize;
        for c in s[from..to].chars() {
            if c == '\t' {
                let n = TAB_COLUMNS - (col % TAB_COLUMNS);
                out.extend(std::iter::repeat_n(' ', n));
                col += n;
            } else {
                out.push(c);
                col += char_cols(c);
            }
        }
        (Cow::Owned(out), col_start)
    }

    /// Replace the text between two cursors, returning the removed text and the
    /// cursor that ends up just after the inserted text.
    pub fn replace(&mut self, range: Range<Cursor>, insert: &str) -> (String, Cursor) {
        // Anchor the rescan on the start of the first affected line. An edit
        // beginning on that line never moves the line's own start offset, so
        // the value read before the splice stays valid afterwards.
        let first_line = self.clamp(range.start).line;
        let start_of_line = self.line_starts[first_line];
        let a = self.byte_of(range.start);
        let b = self.byte_of(range.end).max(a);
        let removed = self.text[a..b].to_string();
        self.text.replace_range(a..b, insert);
        self.reindex_from(first_line, start_of_line);
        let end = self.cursor_of(a + insert.len());
        (removed, end)
    }

    /// Rebuild the line index for `line` and everything after it.
    ///
    /// `start_byte` must be the byte offset where `line` begins.
    fn reindex_from(&mut self, line: usize, start_byte: usize) {
        self.version += 1;
        let tail = scan(&self.text[start_byte..]);
        self.simple = tail.simple;

        let mut starts = tail.line_starts;
        for s in &mut starts {
            *s += start_byte;
        }
        self.line_starts.truncate(line);
        self.line_starts.extend(starts);

        if self.max_line >= line {
            // The previous widest line was inside the region we just rescanned,
            // so the surviving prefix needs a pass to find a new maximum.
            self.max_cols = 0;
            self.max_line = line;
            for l in 0..line {
                let c = self.line_cols(l);
                if c > self.max_cols {
                    self.max_cols = c;
                    self.max_line = l;
                }
            }
        }
        if tail.max_cols > self.max_cols {
            self.max_cols = tail.max_cols;
            self.max_line = line + tail.max_line;
        }
    }

    /// Replace the whole text, e.g. after loading a file or running a transform.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        let idx = scan(&self.text);
        self.line_starts = idx.line_starts;
        self.max_cols = idx.max_cols;
        self.max_line = idx.max_line;
        self.simple = idx.simple;
        self.version += 1;
    }
}

/// Round `byte` down to the nearest UTF-8 character boundary.
fn floor_char_boundary(s: &str, mut byte: usize) -> usize {
    if byte >= s.len() {
        return s.len();
    }
    while byte > 0 && !s.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

/// Direction of a run of edits, used to merge consecutive keystrokes into a
/// single undo step.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EditDir {
    /// Typing forward: the replaced range grows to the right.
    Fwd,
    /// Backspacing: the replaced range grows to the left.
    Back,
}

/// One undoable edit: `[start, end)` was replaced by `inserted`, and whatever
/// was there before was `removed`.
#[derive(Clone, Debug)]
struct UndoEntry {
    group: u64,
    start: Cursor,
    end: Cursor,
    removed: String,
    inserted: String,
    dir: EditDir,
}

/// Per-frame layout numbers, so the geometry is computed once and reused by
/// pointer hit testing, painting and caret placement.
struct Metrics {
    font: FontId,
    char_w: f32,
    row_h: f32,
    /// First and one-past-last visible display column.
    col_start: usize,
    col_end: usize,
    /// Top-left of the (scrolled) content in screen coordinates.
    origin: Pos2,
    /// Visible region in screen coordinates.
    viewport: Rect,
}

/// The `NSTextView` role: cursor, selection, scrolling, undo and painting.
pub struct CodeEditor {
    buffer: TextBuffer,
    cursor: Cursor,
    /// Selection anchor; equal to `cursor` when nothing is selected.
    anchor: Cursor,
    /// Last known scroll offset, mirrored from the scroll area's state.
    scroll: Vec2,
    /// Set when the viewport should be moved to reveal the caret.
    reveal_cursor: bool,
    focus_request: bool,
    /// Selection origin while the pointer is dragging.
    drag_origin: Option<Cursor>,
    blink: f32,
    blink_on: bool,
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    /// Edit group. Bumped whenever the caret jumps, so a run of typing collapses
    /// into one undo step.
    group: u64,
    /// Rows that fit in the viewport, refreshed every frame.
    viewport_rows: usize,
    /// Time and line of the previous click, used to detect triple clicks.
    last_click: Option<(f64, usize)>,
    max_undo: usize,
}

impl CodeEditor {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            buffer: TextBuffer::new(text),
            cursor: Cursor::START,
            anchor: Cursor::START,
            scroll: Vec2::ZERO,
            reveal_cursor: true,
            focus_request: false,
            drag_origin: None,
            blink: 0.0,
            blink_on: true,
            undo: Vec::new(),
            redo: Vec::new(),
            group: 0,
            viewport_rows: 1,
            last_click: None,
            max_undo: 200,
        }
    }

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Replace the document, e.g. after a file load or a Format / Minify run.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.buffer.set_text(text);
        self.cursor = Cursor::START;
        self.anchor = Cursor::START;
        self.undo.clear();
        self.redo.clear();
        self.reveal_cursor = true;
    }

    /// Ask the editor to take keyboard focus on its next frame.
    pub fn request_focus(&mut self) {
        self.focus_request = true;
    }

    /// True when the user has a non-empty selection.
    pub fn has_selection(&self) -> bool {
        self.anchor != self.cursor
    }

    /// The selected range, normalised so `start <= end`.
    pub fn selection(&self) -> (Cursor, Cursor) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    /// The selected range as a `Range`, so an empty selection covers nothing.
    pub fn selection_range(&self) -> Range<Cursor> {
        let (a, b) = self.selection();
        a..b
    }

    /// Selected text; empty when nothing is selected.
    pub fn selected_text(&self) -> String {
        let (a, b) = self.selection();
        if a == b {
            return String::new();
        }
        self.buffer.text()[self.buffer.byte_of(a)..self.buffer.byte_of(b)].to_string()
    }

    // ---------------------------------------------------------------- editing

    /// Apply an edit and record it as one undo step.
    fn edit(&mut self, range: Range<Cursor>, insert: &str, dir: EditDir, group: Option<u64>) {
        let a = self.buffer.clamp(range.start);
        let b = self.buffer.clamp(range.end);
        let group = group.unwrap_or(self.group);
        let (removed, end) = self.buffer.replace(a..b, insert);
        self.push_undo(UndoEntry {
            group,
            start: a,
            end,
            removed,
            inserted: insert.to_string(),
            dir,
        });
        self.cursor = end;
        self.anchor = end;
        self.reveal_cursor = true;
        self.drag_origin = None;
    }

    /// Begin a new edit group so the next edit is its own undo step.
    fn break_group(&mut self) -> u64 {
        self.group += 1;
        self.group
    }

    fn push_undo(&mut self, entry: UndoEntry) {
        let merge = match self.undo.last() {
            Some(last) if last.group == entry.group && last.dir == entry.dir => match entry.dir {
                EditDir::Fwd => last.end == entry.start,
                EditDir::Back => last.start == entry.end,
            },
            _ => false,
        };
        if merge {
            let last = self.undo.last_mut().expect("checked above");
            match entry.dir {
                EditDir::Fwd => {
                    last.end = entry.end;
                    last.inserted.push_str(&entry.inserted);
                }
                EditDir::Back => {
                    last.start = entry.start;
                    last.removed = entry.removed + &last.removed;
                }
            }
        } else {
            self.undo.push(entry);
            if self.undo.len() > self.max_undo {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// Undo the last edit group.
    pub fn undo(&mut self) {
        let Some(entry) = self.undo.pop() else { return };
        let (removed, _) = self.buffer.replace(entry.start..entry.end, &entry.removed);
        let redone_end = self
            .buffer
            .cursor_of(self.buffer.byte_of(entry.start) + entry.removed.len());
        self.cursor = entry.start;
        self.anchor = entry.start;
        self.redo.push(UndoEntry {
            group: entry.group,
            start: entry.start,
            end: redone_end,
            removed,
            inserted: entry.inserted,
            dir: entry.dir,
        });
        self.reveal_cursor = true;
        self.group += 1;
    }

    /// Redo the last undone edit group.
    pub fn redo(&mut self) {
        let Some(entry) = self.redo.pop() else { return };
        let (removed, end) = self.buffer.replace(entry.start..entry.end, &entry.inserted);
        self.undo.push(UndoEntry {
            group: entry.group,
            start: entry.start,
            end,
            removed,
            inserted: entry.inserted,
            dir: entry.dir,
        });
        self.cursor = end;
        self.anchor = end;
        self.reveal_cursor = true;
        self.group += 1;
    }

    // ---------------------------------------------------------------- movement

    fn set_cursor(&mut self, c: Cursor, extend: bool) {
        let c = self.buffer.clamp(c);
        if c == self.cursor && (!extend || self.anchor == c) {
            return;
        }
        self.cursor = c;
        if !extend {
            self.anchor = c;
        }
        self.reveal_cursor = true;
        self.group += 1;
    }

    /// Move the caret vertically, keeping the display column where it fits.
    fn move_vertical(&mut self, delta: isize, extend: bool) {
        let last = self.buffer.line_count() - 1;
        let line = (self.cursor.line as isize + delta).clamp(0, last as isize) as usize;
        let col = self.cursor.col.min(self.buffer.line_cols(line));
        self.set_cursor(Cursor { line, col }, extend);
    }

    fn doc_start(&self) -> Cursor {
        Cursor::START
    }

    fn doc_end(&self) -> Cursor {
        let line = self.buffer.line_count() - 1;
        Cursor {
            line,
            col: self.buffer.line_cols(line),
        }
    }

    fn line_start(&self, line: usize) -> Cursor {
        Cursor { line, col: 0 }
    }

    fn line_end(&self, line: usize) -> Cursor {
        Cursor {
            line,
            col: self.buffer.line_cols(line),
        }
    }

    /// Previous word boundary, or one character back when `by_word` is false.
    fn step_back(&self, c: Cursor, by_word: bool) -> Cursor {
        let byte = self.buffer.byte_of(c);
        let text = self.buffer.text();
        if byte == 0 {
            return c;
        }
        let new_byte = if by_word {
            let mut i = byte;
            while i > 0 && is_word(text[..i].chars().next_back().unwrap_or(' ')) {
                i -= 1;
            }
            while i > 0 && !is_word(text[..i].chars().next_back().unwrap_or(' ')) {
                i -= 1;
            }
            i
        } else {
            text[..byte]
                .chars()
                .next_back()
                .map_or(0, |ch| byte - ch.len_utf8())
        };
        self.buffer.cursor_of(new_byte)
    }

    /// Next word boundary, or one character forward when `by_word` is false.
    fn step_forward(&self, c: Cursor, by_word: bool) -> Cursor {
        let byte = self.buffer.byte_of(c);
        let text = self.buffer.text();
        if byte >= text.len() {
            return c;
        }
        let new_byte = if by_word {
            let mut i = byte;
            while i < text.len() && !is_word(text[i..].chars().next().unwrap_or(' ')) {
                i += 1;
            }
            while i < text.len() && is_word(text[i..].chars().next().unwrap_or(' ')) {
                i += 1;
            }
            i
        } else {
            text[byte..]
                .chars()
                .next()
                .map_or(text.len(), |ch| byte + ch.len_utf8())
        };
        self.buffer.cursor_of(new_byte)
    }

    /// Delete the selection, or the text around the caret.
    fn delete(&mut self, by_word: bool, backwards: bool) {
        if self.has_selection() {
            let (a, b) = self.selection();
            self.group += 1;
            self.edit(a..b, "", EditDir::Fwd, Some(self.group));
            return;
        }
        let c = self.cursor;
        if backwards {
            let target = self.step_back(c, by_word);
            self.edit(target..c, "", EditDir::Back, None);
        } else {
            let target = self.step_forward(c, by_word);
            self.edit(c..target, "", EditDir::Fwd, None);
        }
    }

    fn select_all(&mut self) {
        self.anchor = self.doc_start();
        self.cursor = self.doc_end();
        self.reveal_cursor = true;
        self.group += 1;
    }

    /// Word boundaries around `c` for double-click selection.
    fn word_range(&self, c: Cursor) -> (Cursor, Cursor) {
        let text = self.buffer.text();
        let line_start = self.buffer.byte_of(self.line_start(c.line));
        let line_end = self.buffer.byte_of(self.line_end(c.line));
        let caret = self.buffer.byte_of(c);
        let mut start = caret.clamp(line_start, line_end);
        while start > line_start {
            let prev = text[..start].chars().next_back().unwrap_or(' ');
            if !is_word(prev) {
                break;
            }
            start -= prev.len_utf8();
        }
        let mut end = start;
        while end < line_end {
            let ch = text[end..].chars().next().unwrap_or(' ');
            if !is_word(ch) {
                break;
            }
            end += ch.len_utf8();
        }
        (self.buffer.cursor_of(start), self.buffer.cursor_of(end))
    }

    fn select_word(&mut self, c: Cursor) {
        let (a, b) = self.word_range(c);
        self.anchor = a;
        self.cursor = b;
        self.reveal_cursor = true;
        self.group += 1;
    }

    fn select_line(&mut self, c: Cursor) {
        self.anchor = self.line_start(c.line);
        self.cursor = if c.line + 1 < self.buffer.line_count() {
            self.line_start(c.line + 1)
        } else {
            self.line_end(c.line)
        };
        self.reveal_cursor = true;
        self.group += 1;
    }

    // ------------------------------------------------------------------ frame

    /// Render one frame. Returns true when the text changed.
    pub fn show(&mut self, ui: &mut egui::Ui, font: FontId) -> bool {
        let before = self.buffer.version();
        self.frame(ui, font);
        self.buffer.version() != before
    }

    fn frame(&mut self, ui: &mut egui::Ui, font: FontId) {
        let id = ui.id().with("code_editor");
        if self.focus_request {
            ui.memory_mut(|m| m.request_focus(id));
            self.focus_request = false;
        } else if ui.memory(|m| m.focused()).is_none() {
            // Nothing else wants the keyboard, so take it. Without this, typing
            // in the Text or Split tab is dropped until the user clicks first.
            // Any other widget that asks for focus later in the same pass wins,
            // because `request_focus` simply overwrites.
            ui.memory_mut(|m| m.request_focus(id));
        }

        let char_w = ui.fonts(|f| f.glyph_width(&font, '0')).max(1.0);
        let row_h = ui.fonts(|f| f.row_height(&font)).max(8.0);
        let available = ui.available_size();
        let viewport_size = Vec2::new(available.x.max(64.0), available.y.max(64.0));
        self.viewport_rows = (viewport_size.y / row_h).floor().max(1.0) as usize;

        let line_count = self.buffer.line_count();
        let content_w = (self.buffer.max_cols() as f32 * char_w + PAD_X * 2.0).max(viewport_size.x);
        let content_h = (line_count as f32 * row_h).max(viewport_size.y);

        let mut scroll = egui::ScrollArea::both().id_salt(id).auto_shrink([false, false]);
        if self.reveal_cursor {
            let (oy, ox) = self.reveal_offsets(char_w, row_h, viewport_size);
            scroll = scroll.vertical_scroll_offset(oy).horizontal_scroll_offset(ox);
            self.reveal_cursor = false;
        }

        let out = scroll.show(ui, |ui| {
            ui.allocate_space(Vec2::new(content_w, content_h));

            let origin = ui.max_rect().min;
            let viewport = ui.clip_rect().intersect(ui.max_rect());
            let m = Metrics {
                font: font.clone(),
                char_w,
                row_h,
                col_start: (((viewport.min.x - origin.x) - PAD_X) / char_w)
                    .floor()
                    .max(0.0) as usize,
                col_end: (((viewport.max.x - origin.x) - PAD_X) / char_w)
                    .ceil()
                    .max(0.0) as usize,
                origin,
                viewport,
            };

            let response = ui.interact(viewport, id, Sense::click_and_drag());
            self.handle_pointer(ui, response, &m, id);
            if ui.memory(|mem| mem.has_focus(id)) {
                self.handle_keys(ui);
                // Selection geometry depends on the caret, so paint afterwards.
            }
            self.paint(ui, &m);
        });

        self.scroll = out.state.offset;

        if ui.memory(|mem| mem.has_focus(id)) {
            self.blink += ui.input(|i| i.stable_dt.min(0.1));
            if self.blink >= BLINK_PERIOD {
                self.blink -= BLINK_PERIOD;
                self.blink_on = !self.blink_on;
                ui.ctx().request_repaint();
            }
        }
    }

    /// Scroll offsets that reveal the caret with the smallest possible move.
    fn reveal_offsets(&self, char_w: f32, row_h: f32, viewport: Vec2) -> (f32, f32) {
        let y = self.cursor.line as f32 * row_h;
        let x = self.cursor.col as f32 * char_w + PAD_X;
        let mut oy = self.scroll.y;
        if y < oy {
            oy = y;
        } else if y + row_h > oy + viewport.y {
            oy = y + row_h - viewport.y;
        }
        let mut ox = self.scroll.x;
        if x < ox {
            ox = x;
        } else if x + char_w > ox + viewport.x {
            ox = x + char_w - viewport.x;
        }
        (oy.max(0.0), ox.max(0.0))
    }

    /// Turn a pointer position into a caret position.
    fn cursor_at(&self, m: &Metrics, pos: Pos2) -> Cursor {
        let line = (((pos.y - m.origin.y).max(0.0)) / m.row_h).floor() as usize;
        let line = line.min(self.buffer.line_count() - 1);
        let col = (((pos.x - m.origin.x) - PAD_X).max(0.0) / m.char_w).round() as usize;
        self.buffer.clamp(Cursor { line, col })
    }

    fn handle_pointer(&mut self, ui: &mut egui::Ui, response: egui::Response, m: &Metrics, id: egui::Id) {
        let primary = egui::PointerButton::Primary;
        if response.clicked_by(primary) {
            // Take keyboard focus, otherwise typing is dropped because
            // `handle_keys` only runs for the focused widget.
            ui.memory_mut(|mem| mem.request_focus(id));
            let pos = response.interact_pointer_pos().unwrap_or(m.viewport.center());
            let c = self.cursor_at(m, pos);
            let now = ui.input(|i| i.time);
            let line_click = self
                .last_click
                .filter(|(t, line)| now - *t < TRIPLE_CLICK_SECONDS && *line == c.line);
            self.last_click = Some((now, c.line));
            self.group += 1;
            if line_click.is_some() {
                self.select_line(c);
            } else if ui.input(|i| i.pointer.button_double_clicked(primary)) {
                self.select_word(c);
            } else {
                self.cursor = c;
                self.anchor = c;
                self.reveal_cursor = true;
            }
            self.drag_origin = Some(self.selection().0);
            self.restart_blink();
        } else if response.dragged_by(primary) {
            if let Some(origin) = self.drag_origin {
                ui.memory_mut(|mem| mem.request_focus(id));
                let pos = response.interact_pointer_pos().unwrap_or(m.viewport.center());
                self.cursor = self.cursor_at(m, pos);
                self.anchor = origin;
            }
        } else if !response.dragged() {
            self.drag_origin = None;
        }
    }

    fn restart_blink(&mut self) {
        self.blink = 0.0;
        self.blink_on = true;
    }

    fn handle_keys(&mut self, ui: &mut egui::Ui) {
        let events = ui.input(|i| i.events.clone());
        let mut keys = Vec::new();
        let mut handled = false;

        for event in events {
            match event {
                Event::Text(text) if !text.is_empty() => {
                    let range = self.selection_range();
                    self.edit(range, &text, EditDir::Fwd, None);
                    handled = true;
                }
                Event::Paste(text) if !text.is_empty() => {
                    let range = self.selection_range();
                    let group = self.break_group();
                    self.edit(range, &text, EditDir::Fwd, Some(group));
                    handled = true;
                }
                Event::Copy | Event::Cut => {
                    let text = self.selected_text();
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                        if matches!(event, Event::Cut) {
                            let range = self.selection_range();
                            self.edit(range, "", EditDir::Fwd, None);
                            handled = true;
                        }
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => keys.push((key, modifiers)),
                _ => {}
            }
        }

        for (key, mods) in keys {
            // The modifiers carried by the event itself are used, not the ones
            // on the frame, so a key press is always interpreted the way it was
            // reported.
            let ctrl = mods.ctrl;
            let shift = mods.shift;
            let alt = mods.alt;
            // Editor keys are consumed so egui's Tab focus traversal and the
            // surrounding scroll area do not also react to them. App-level
            // shortcuts (Ctrl+S, Ctrl+O, ...) are deliberately left alone.
            if self.handle_key(ui, key, ctrl, shift, alt) {
                ui.input_mut(|i| i.consume_key(mods, key));
                handled = true;
            }
        }

        if handled {
            self.restart_blink();
        }
    }

    fn handle_key(&mut self, ui: &mut egui::Ui, key: Key, ctrl: bool, shift: bool, alt: bool) -> bool {
        if ctrl && !alt {
            match key {
                Key::A => {
                    self.select_all();
                    return true;
                }
                Key::C => {
                    let text = self.selected_text();
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                    }
                    return true;
                }
                Key::X => {
                    let text = self.selected_text();
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                        let range = self.selection_range();
                        self.edit(range, "", EditDir::Fwd, None);
                    }
                    return true;
                }
                Key::V => return true, // delivered as Event::Paste
                Key::Z => {
                    if shift {
                        self.redo();
                    } else {
                        self.undo();
                    }
                    return true;
                }
                Key::Y => {
                    self.redo();
                    return true;
                }
                _ => {}
            }
        }

        match key {
            Key::ArrowLeft => {
                let target = self.step_back(self.cursor, ctrl);
                self.set_cursor(target, shift);
                true
            }
            Key::ArrowRight => {
                let target = self.step_forward(self.cursor, ctrl);
                self.set_cursor(target, shift);
                true
            }
            Key::ArrowUp => {
                if ctrl {
                    self.set_cursor(self.doc_start(), shift);
                } else {
                    self.move_vertical(-1, shift);
                }
                true
            }
            Key::ArrowDown => {
                if ctrl {
                    self.set_cursor(self.doc_end(), shift);
                } else {
                    self.move_vertical(1, shift);
                }
                true
            }
            Key::Home => {
                let target = if ctrl {
                    self.doc_start()
                } else {
                    self.line_start(self.cursor.line)
                };
                self.set_cursor(target, shift);
                true
            }
            Key::End => {
                let target = if ctrl {
                    self.doc_end()
                } else {
                    self.line_end(self.cursor.line)
                };
                self.set_cursor(target, shift);
                true
            }
            Key::PageUp => {
                self.move_vertical(-(self.viewport_rows.max(2) as isize - 1), shift);
                true
            }
            Key::PageDown => {
                self.move_vertical(self.viewport_rows.max(2) as isize - 1, shift);
                true
            }
            Key::Enter => {
                let range = self.selection_range();
                let group = self.break_group();
                self.edit(range, "\n", EditDir::Fwd, Some(group));
                true
            }
            Key::Tab => {
                if self.has_selection() {
                    let (a, b) = self.selection();
                    let group = self.break_group();
                    let indent = " ".repeat(TAB_COLUMNS);
                    let mut out = String::new();
                    for line in a.line..=b.line {
                        out.push_str(&indent);
                        if line < b.line {
                            out.push('\n');
                        }
                    }
                    self.edit(a..b, &out, EditDir::Fwd, Some(group));
                } else {
                    let c = self.cursor;
                    let n = TAB_COLUMNS - (c.col % TAB_COLUMNS);
                    self.edit(c..c, &" ".repeat(n), EditDir::Fwd, None);
                }
                true
            }
            Key::Backspace => {
                self.delete(ctrl, true);
                true
            }
            Key::Delete => {
                self.delete(ctrl, false);
                true
            }
            Key::Escape => {
                if self.has_selection() {
                    let (a, _) = self.selection();
                    self.set_cursor(a, false);
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    // ---------------------------------------------------------------- painting

    fn paint(&mut self, ui: &mut egui::Ui, m: &Metrics) {
        let painter = ui.painter();
        let visuals = ui.visuals();
        let text_color = if ui.is_enabled() {
            visuals.text_color()
        } else {
            visuals.weak_text_color()
        };

        let line_count = self.buffer.line_count();
        let first = (((m.viewport.min.y - m.origin.y) / m.row_h).floor().max(0.0) as usize)
            .min(line_count);
        let last = (((m.viewport.max.y - m.origin.y) / m.row_h).ceil().max(0.0) as usize)
            .min(line_count)
            .max(first);
        if first >= last {
            return;
        }

        // Selection background.
        let (sel_start, sel_end) = self.selection();
        if sel_start != sel_end {
            for line in sel_start.line.max(first)..(sel_end.line + 1).min(last) {
                let line_cols = self.buffer.line_cols(line);
                let c0 = if line == sel_start.line {
                    sel_start.col.min(line_cols)
                } else {
                    0
                };
                let c1 = if line == sel_end.line {
                    sel_end.col.min(line_cols)
                } else {
                    line_cols
                };
                if c1 <= c0 {
                    continue;
                }
                let y = m.origin.y + line as f32 * m.row_h;
                let x0 = m.origin.x + PAD_X + c0 as f32 * m.char_w;
                let x1 = m.origin.x + PAD_X + c1 as f32 * m.char_w;
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + m.row_h)),
                    0.0,
                    visuals.selection.bg_fill,
                );
            }
        }

        // Only the rows intersecting the viewport are shaped. This is the egui
        // counterpart of NSTextView's `allowsNonContiguousLayout`.
        for line in first..last {
            let (slice, start_col) = self.buffer.visible_slice(line, m.col_start, m.col_end);
            if slice.is_empty() {
                continue;
            }
            painter.text(
                Pos2::new(
                    m.origin.x + PAD_X + start_col as f32 * m.char_w,
                    m.origin.y + line as f32 * m.row_h,
                ),
                Align2::LEFT_TOP,
                slice.as_ref(),
                m.font.clone(),
                text_color,
            );
        }

        // Caret.
        if self.blink_on && self.cursor.line >= first && self.cursor.line < last {
            let y = m.origin.y + self.cursor.line as f32 * m.row_h;
            let x = m.origin.x + PAD_X + self.cursor.col as f32 * m.char_w;
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(2.0, m.row_h)),
                0.0,
                visuals.text_color(),
            );
        }
    }
}

/// Whether `ch` is part of a JSON identifier / number, for word motions.
fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}
