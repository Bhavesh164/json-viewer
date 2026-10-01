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

**Status: DONE on macOS.** `needsRawTextSync` / `syncRawTextIfNeeded()`; the editor owns the text while typing.
**Impact:** the largest remaining per-keystroke cost, now removed.

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

### What was implemented

- `JSONDocumentTextSource` (`JSONDocumentModel.swift`) is the pull interface. The
  native editor's `Coordinator` conforms and is held **weakly**; more than one may be
  registered, because a tab switch can briefly leave two alive.
- `markEditedFromEditor(lineCount:characterCount:)` is called from `textDidChange`. It
  records that a sync is pending and publishes the counts the editor already knew. It
  does **not** read the buffer.
- `syncRawTextIfNeeded()` is called by everything that consumes the document: the
  live parse, all transforms, every Copy variant, Save, Open, Paste, search, and the
  tab switch. The pending-debounce work items read `rawText` *inside* the work, after
  the sync, so the copy happens once per idle period.
- `updateNSView` now returns early when `hasPendingTextSync` is set, not only when
  `isUpdatingFromTextView` is. This matters: without it the model would push its stale
  copy back into the buffer and revert what was just typed.
- Views must not read `rawText` to test emptiness, because it lags on purpose. They
  read the published `isDocumentEmpty` instead.

Measured on 24 MB / 1,080,004 lines: a whole-buffer readback costs **24.1 ms**; the
per-edit line accounting costs **0.00005 ms**. That is the ~500,000x gap the item is
about.

### How to verify

Signposts around `textDidChange`, or a quick Instruments allocation profile while
typing into a large document. Expect the per-keystroke allocation to collapse from
one full-document buffer to a small one.

---

## 2. Parse off the main thread, or at least measure it

**Status: DONE on macOS** — option 1. **Impact:** the visible stall in Split view is gone.

### Measured on macOS, 24 MB / 1,080,004 lines

The Linux figure quoted below (~50 ms) does **not** transfer: on macOS a full
parse-plus-tree-build of this document takes **~1.0 s**, so the main-thread stall was
roughly twenty times worse than the note assumed.

Worst main-loop gap (what a user perceives as a freeze), sampled with a 5 ms timer on
the main run loop:

| | worst gap |
| --- | --- |
| `openFile` (explicit user action, parses on the main actor) | 0.99 s |
| Split-mode live parse, debounce 1.5 s | **0.069 s** |

The live-parse gap is the cost of applying an already-built tree, not parsing.

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

### What was implemented

`parseDocument(_:options:)` is a pure `nonisolated static` function: it trims, parses,
builds the tree, and builds the node lookup index without touching main-actor state.
`runLiveParse()` runs it in a `Task.detached` and only hops back to apply the result.

- The settings are snapshotted into a `Sendable` `ParseOptions` first.
- `ParseOutcome` and `TreeIndex` are `@unchecked Sendable`, which is sound because
  `JSONValue` is a value type and `JSONNode` is immutable once built.
- A `parseSerial` counter discards a result whose text is no longer current, mirroring
  the search generation counter. The tree indexing is iterative, so a deeply nested
  document cannot overflow the stack off-thread.
- `parseAndBuildTree` stays **synchronous** on purpose: the tab switch, the transforms
  and the tests all need the tree to be correct on return.

### How to verify

Instruments Time Profiler on the main thread while typing in Split view with a
large document loaded. The debounce now fires on a size-scaled delay (item 3), not a
fixed 0.35 s, and the spike should be gone entirely.

---

## 3. Scale the parse debounce to the document size

**Status: DONE on macOS.** `liveParseDebounceDelay`. **Impact:** rebuilds interrupting steady typing.

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

### What was implemented

`liveParseDebounceDelay` uses the same formula as the Rust port. One macOS-specific
detail: it reads `characterCount`, which had to be made **synchronous** to be usable.
`characterCount` is `NSString.length` — a stored UTF-16 length, not a walk — so it is
cheap enough to measure on every change, and it is now set before the debounced line
count. Using the debounced count would have sized a freshly opened 24 MB document's
debounce from the *previous* document.

`updateTextMetrics` also switched its size probe from `text.count` to
`NSString.length`, because `String.count` walks graphemes — a per-character pass over
the whole document, on the typing path, for a threshold that only needs a size.

### How to verify

Count parses per second of continuous typing, before and after, with a large
document loaded.

---

## 4. Metrics: prefer the incremental count over a rescan

**Status: DONE on macOS.** The counting loop and the *source* of the counts both changed.
**Priority:** was low — it is now part of the typing path by design, because item 1
means the editor supplies these counts instead of a rescan.

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

### What was implemented

The `for byte in text.utf8` loop is gone. `JSONDocumentModel.lineCount(of:)` counts
with `memchr` over the contiguous UTF-8 buffer instead. The warning above was taken
seriously: `String.split` was not used.

Measured on 24 MB / 1,080,004 lines:

| | per call |
| --- | --- |
| old `for byte in text.utf8` loop | 28.8 ms |
| `memchr` counter | **5.9 ms** |
| whole-buffer `String` readback, for comparison | 24.1 ms |

More importantly, the *typing path no longer scans at all*. The `Coordinator` keeps the
line count incrementally, exactly as the Linux `TextBuffer` does:

- `textView(_:shouldChangeTextIn:replacementString:)` adds the newlines in the
  replacement and subtracts the newlines in the replaced range — a scan of the edit,
  not the document. Measured at 0.00005 ms.
- The character count is free: `NSTextStorage.length` is already in hand.
- `expectedLength` is an integrity check. It is the length the buffer should have once
  the in-flight edit lands; if the actual length disagrees, something bypassed
  `shouldChangeTextIn` (an undo, or a programmatic `replaceCharacters`) and the count
  is re-anchored from the buffer instead of being wrong forever.
- The re-anchor is followed by a short cooldown, so a delegate that is skipped
  *systematically* cannot turn every keystroke into a full scan. The failure mode is a
  bounded, briefly stale line count rather than a per-keystroke stall.

`markEditedFromEditor` cancels any in-flight measurement, so a background count of the
*previous* text cannot land on top of the editor's counts for the current one.

---

## 5. Deduplicate repeated object keys when building the tree

**Status: DONE on macOS.** **Impact:** a view concern, not really throughput.

**Decision taken:** implement the Linux behaviour. It matches what a JSON consumer
would resolve, it reduces the node count, and leaving the text untouched means
Format / Minify / Save still round-trip exactly what was typed.

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

### What was implemented

`JSONValue.parseObject` is **unchanged**: the parser still keeps every pair. The
collapse happens in `JSONNode.buildNode` via `resolvingRepeatedKeys`, so the view
decides what the document means while the parser stays faithful.

One detail the Linux port does not do: the container node's own `value` is built from
the *resolved* pairs rather than the raw ones. Otherwise `displayText` would report
`JSON {3}` for a document that shows one row — the badge and the children would
disagree.

Tests, mirroring the three Linux cases:
- `A repeated object key collapses to one tree row` / `... to the last value`
- `Duplicate keys keep their position and do not disturb siblings`
- `Duplicate keys in an array element do not collapse across elements`
- `Parser keeps every repeated object key` — pins that the parser is still faithful

The pre-existing test that asserted three separate rows for
`{"count":3,"count":23423,"count":23423}` was replaced; it pinned the old behaviour
this item deliberately changes.

---

## 6. Emptying the document must clear the tree

**Status: DONE on macOS.** `discardTree()`. **Impact:** correctness, not throughput.

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

### What was implemented

`discardTree()` drops the value, tree, selection, property grid, both expanded sets,
the visible rows, the stale `parseError` and all search state, and bumps `treeVersion`
so a row from the discarded tree can never be reused by identity. It is called from the
empty branch of `applyParse`, and `clearText()` now uses it too.

The distinction in the note above is preserved and tested: an invalid but non-empty
document keeps the last good tree, while an empty one discards it.

Tests: 11 assertions covering the empty and whitespace-only cases, plus `treeVersion`
being bumped and `clearText` / Split-mode clearing dropping the tree immediately.

---

## Summary

All six items are implemented on macOS. Two changes beyond the items as written turned
out to be necessary:

1. `updateNSView` had to learn about `hasPendingTextSync`. Without it the model pushes
   its stale copy into the text view and reverts the keystroke that was just typed —
   the item-1 change is only safe together with this.
2. `characterCount` had to become synchronous for the item-3 debounce to size itself
   from the current document.

One pre-existing bug was fixed to make verification possible: the search tests asserted
synchronously against a search that completes on another thread, so 14 of them failed
on a clean checkout before any of this work.

The suite is now **210 assertions, all passing**, with coverage added for each item and
for the failure modes that are easy to get wrong:

- the incremental line count, checked against a full byte scan for appends, newline
  deletes, Enter, multi-line paste, select-all-delete, undo-style delete-then-retype and
  ranges spanning lines;
- editor lifetime, since item 1 moves ownership of the text: two editors registered at
  once, focus moving between them, re-registration, an unregister that flushes, and a
  source deallocated without unregistering;
- the editor's counts not being reused for a document set from somewhere else, and a
  stale background measurement not landing on top of them;
- a stale live-parse result being discarded rather than applied.

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
| Line counting via `memchr`, not a byte loop | `JSONDocumentModel.swift` `lineCount(of:)` |
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
