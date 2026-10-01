# Performance

Cross-platform notes for JSON Viewer.

This file records optimizations that were implemented in the **Linux / egui port**
and still apply to the **macOS** app. Each item states what is slow today, why, the
change, and how to confirm it worked.

**Scope rule:** an optimization is only listed here if macOS does *not* already do
it. Work that is already in place is collected under
[Already done on macOS](#already-done-on-macos-do-not-re-do-these) so it is not
wasted effort, and so the reasoning behind the remaining items is visible.

**On the numbers:** every measurement quoted below was taken on Linux, in a
release build, against a 7.7 MB / 400,000-line document. The *mechanisms* are
what transfer; the macOS numbers are not measured yet, so treat each item as
"measure before and after" rather than as a predicted result.

---

## 1. Stop copying the whole document on every keystroke

**Status:** not done on macOS. **Impact:** the largest remaining per-keystroke cost.

### What happens now

`NativeCodeEditor.Coordinator.textDidChange`
(`Sources/JSONViewer/Views/TextEditorView.swift:437`) does:

```swift
parent.text = tv.string
```

`NSTextView.string` materialises a **new Swift `String` containing the entire
document** on every call. Assigning it to the `@Published var rawText`
(`Sources/JSONViewerCore/JSONDocumentModel.swift:15`) therefore allocates and
copies the whole file once per character typed. For a 7.7 MB document that is
7.7 MB of allocation and copy for every keypress, and the old buffer is released
each time.

Note that the *readback* direction is already optimised: `updateNSView` returns
early via `isUpdatingFromTextView` and compares `textStorage.length` rather than
building strings, so the expensive part is the write path only.

### Why it matters

It is the one unavoidable-looking cost that is actually avoidable. Nothing
downstream of `rawText.didSet` needs the full string on every keystroke — the
debounced metric update and the split-mode live parse only need to know *that*
the text changed, and the full value is not read until the debounce fires.

### The change

Let the text view own the text while the user is typing, and sync the model only
when something actually consumes it. The Linux port does this as:

- a `pendingTextChange: Bool` flag set from the delegate, and
- `syncRawTextIfNeeded()`, called immediately before the work that reads
  `rawText`: the debounced parse, Format / Minify / Stringify / JSON↔Python,
  every Copy variant, Save, and the tab switch.

In Swift this maps to keeping a `needsRawTextSync` flag on the model, set from
`rawText`'s current source, and draining it at those same call sites. The
deferred `DispatchWorkItem`s should capture the flag, not the string, so the
copy happens once per idle period rather than once per keypress.

Care needed: the debounced work items currently capture `let text = rawText` at
schedule time (`JSONDocumentModel.swift:122`, `:210`). They must instead read
`rawText` *inside* the work, after the sync, or they will copy the value on every
keystroke and give back the cost that was removed.

### How to verify

Signposts around `textDidChange`, or a quick Instruments allocation profile while
typing into a large document. Expect the per-keystroke allocation to collapse from
one full-document buffer to a small one.

---

## 2. Parse off the main thread, or at least measure it

**Status:** not done on macOS. **Impact:** a visible stall in Split view.

### What happens now

`scheduleLiveParseIfInSplitMode()`
(`Sources/JSONViewerCore/JSONDocumentModel.swift:207`) debounces, then calls
`parseAndBuildTree` synchronously from a `DispatchQueue.main` block
(`:215`).

`JSONDocumentModel` is annotated `@MainActor` (`:13`), and `parseAndBuildTree`
mutates published state, so the whole parse-plus-tree-build — for every node,
every `id`, every `path` string — runs on the main thread. On the Linux port that
step measured **~50 ms for an 8 MB document** and varied between ~50 ms and
several seconds depending on which recovery path the input took. Even the fast
case is long enough to drop frames visibly every time the debounce fires.

### The change

Two options, in increasing order of effort. Either is a real improvement; measure
before choosing.

1. **Move the work off the main actor.** `JSONValue` is already a value type and
   `JSONNode` is `@unchecked Sendable`, and search already proves the pattern
   works via `Task.detached` (`:582`). Build the tree into locals off-thread, then
   assign the published state in one hop back to the main actor, guarded by the
   same kind of generation counter used for search so a stale result is discarded.

2. **At minimum, measure it** and make the cost visible. A stall nobody has
   measured tends to be attributed to something else.

Be aware `format`, `minify` and the transforms are on the same code path, so this
helps those too.

### How to verify

Instruments Time Profiler on the main thread while typing in Split view with a
large document loaded. The debounce fires ~0.35 s after the last keystroke; that
is the spike to look for.

---

## 3. Scale the parse debounce to the document size

**Status:** not done on macOS. **Impact:** rebuilds interrupting steady typing.

### What happens now

A fixed 350 ms delay (`JSONDocumentModel.swift:215`). A user typing steadily
pauses for more than 350 ms often enough that the parse fires *between*
keystrokes, so a large document is re-parsed repeatedly while they are still
typing.

### The change

Make the delay a function of document size, so a big file is only rebuilt after a
real pause. The Linux port uses:

```rust
// 350 ms up to 1 MB, then +250 ms per MB, capped at 1.5 s
let mb = character_count as f64 / (1024.0 * 1024.0);
let extra = (mb.max(0.0) * 250.0) as u128;
350 + extra.min(1150)
```

The metrics are already available and already maintained off the main thread
(`updateTextMetrics`), so the size is known without scanning the document. Tune the
constants against the measurement from item 2 — if the parse turns out to be cheap
enough to leave the main thread, this item matters much less.

### How to verify

Count parses per second of continuous typing, before and after, with a large
document loaded.

---

## 4. Metrics: prefer the incremental count over a rescan

**Status:** partially done — the *scheduling* is already right, one detail is not.
**Priority:** low, because it is already off the main thread behind a debounce.

`updateTextMetrics` (`:120`) is already well designed: a synchronous fast path
below 15 KB, and a cancellable `DispatchWorkItem` on a global queue above it, so
the per-keystroke rescan is already avoided for large documents. The remaining
detail is the counting loop itself:

```swift
for byte in text.utf8 {
    if byte == 0x0A { count += 1 }
}
```

`String.UTF8View` iteration is a per-byte loop. On the Linux port the equivalent
count was one of the two largest per-keystroke costs on an 8 MB file. A caution
before optimising it: **do not assume `String` splitting is memchr-backed here.**
Measured on 8 MB, `text.split("\n").count()` came out *slower* (31.6 ms) than the
plain byte loop (23.3 ms), so reaching for `components(separatedBy:)` would have
made it worse. The only clear win is to not scan at all:

- take the line count from the layout, which already knows where the line breaks
  are — `NSTextStorage` / `NSLayoutManager` can report the number of lines
  without touching the bytes, or
- maintain the count incrementally, the way the Linux `TextBuffer` does with its
  line index (a per-line offset table that is shifted, not rescanned, on each
  edit).

Since this already runs off the main thread behind a debounce, treat it as a
tidy-up rather than a fix.

---

## 5. Deduplicate repeated object keys when building the tree

**Status:** not done on macOS. **Impact:** a view concern, not really throughput.

Not a performance item, but it came out of the same work and it reduces the node
count the tree has to build and lay out, so it is recorded here.

An object may repeat a key, and the editor lets you type one. Both ports keep
every pair in `parseObject`, which means the tree materialises one node per
repetition. The Linux port now collapses a repeated key to its **last** value in
the tree view — the value a consumer would actually see, as `JSON.parse` resolves
it — while leaving the text untouched so Format / Minify / Save round-trip
exactly what was typed. The surviving duplicate keeps the position of the first
occurrence, so sibling keys do not move.

This is a **behaviour change** and diverges from the current Swift core, so it
needs a decision, not just a patch:

- `JSONValue.parseObject` → keep all pairs (the parser should stay faithful), and
- `JSONNode` construction → collapse to the last occurrence per object (the view
  decides what the document means).

Deduplicate per object, not per document: `[{"k":1},{"k":2}]` still shows two `k`
rows. The property grid is a view of the same children, so it follows for free.

Tests on the Linux side that pin the behaviour:
`repeated_object_keys_collapse_to_the_last_value`,
`duplicate_keys_keep_their_position_and_do_not_disturb_siblings`,
`duplicate_keys_in_an_array_element_also_collapse`.

---

## 6. Emptying the document must clear the tree

**Status:** not done on macOS. **Impact:** correctness, not throughput.

Recorded here because it is a real bug present in the macOS build, found while
profiling item 1.

`parseAndBuildTree` (`:219`) returns early when the trimmed text is empty, but
only to show an error. It leaves the previous tree, the previous selection, the
expanded set and the stale `parseError` in place, so deleting everything leaves
the old tree on screen and the status bar still reporting an error from an
earlier invalid state. Confirmed on both ports.

Note the distinction, because the Split-mode behaviour depends on it: a
**non-empty but invalid** document should keep the last good tree, flagged out of
date, so the user does not lose their place mid-edit. An **empty** document has
no interpretation left to preserve, so the tree, the selection, the search
results and the stale error should all be dropped.

The Linux fix is `DocumentModel.discard_tree()`, called from the empty branch,
with `clear()` refactored to use it. Tests:
`emptying_the_document_drops_the_stale_tree` (covers empty and whitespace-only).

---

## Already done on macOS — do not re-do these

Verified present, and deliberately excluded from the items above.

| Optimization | Where |
| --- | --- |
| Non-contiguous text layout — only visible lines are laid out | `TextEditorView.swift:293` `allowsNonContiguousLayout = true` |
| Background layout of off-screen text | `TextEditorView.swift:294` `backgroundLayoutEnabled = true` |
| Avoid whole-string compare on every SwiftUI update | `TextEditorView.swift:380-385` early return plus `textStorage.length` |
| Virtualized tree rows | `TreeViewer.swift:106-107` `LazyVStack` + `ForEach` |
| Search off the main thread, cancellable | `JSONDocumentModel.swift:582` `Task.detached` with `Task.isCancelled` checks (`JSONNode.swift:176`) |
| Search without repeated array concatenation | `JSONNode.swift:173-174` single accumulator, iterative traversal |
| Lazily cached grapheme counts | `JSONNode.swift:12` `lazy var stringCharacterCount` |
| Status metrics debounced off the main thread | `JSONDocumentModel.swift:120-157` |
| Live parse restricted to Split mode | `JSONDocumentModel.swift:207` `guard activeTab == .split` |
| Property grid rebuilt on selection change only | `JSONDocumentModel.swift:748` |
| Spell/grammar/smart-substitution disabled in the editor | `TextEditorView.swift:328-332` |
| Paste converts Python literals to JSON | `TextEditorView.swift:241` `EditorTextView.paste` |

### The Linux numbers these came from

For reference when re-measuring on macOS. Text tab, release build, 1400×900
viewport, 7.7 MB / 400,000-line document:

| | before | after |
| --- | --- | --- |
| Frame cost, idle | 543 ms | 0.69 ms |
| Keystroke frame | 22.3 ms | 0.11 ms |
| Keystroke, 8 MB document, end to end | 62 ms | 3.35 ms |
| 50 MB document, frame cost | 5,219 ms | 0.48 ms |

The 543 ms → 0.69 ms figure is the whole reason the Linux editor is virtualized
at all: egui's `TextEdit::multiline` re-shapes the whole buffer every frame, and
macOS avoids the same cost via `allowsNonContiguousLayout`. macOS does not need
that work; the remaining items above are the ones it still has.
