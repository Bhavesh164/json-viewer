# Windows follow-up

All code items below are complete. Verified with the model test suite and GUI
smoke tests under Wine 8 / Xvfb (x86-64), then scanned with ClamAV.

## Search responsiveness

- [x] Run searches on a cancellable background task so clearing or editing the
      query stays responsive on large JSON documents.
- [x] Traverse nodes with a shared match accumulator to avoid repeatedly
      copying descendant results.
- [x] Show search-in-progress status and discard results from canceled or
      outdated queries. Results are polled from the UI thread on a timer.

## Search and Properties navigation

- [x] When Enter, Go, Next, Previous, or a Properties row selects a match,
      expand its ancestors before scrolling the tree to that node.
- [x] Trigger scrolling after the expanded rows have been rebuilt so
      virtualized tree rows can be found reliably.
- [x] When returning from Text to Viewer with a search match still selected,
      restore the tree scroll position even if the selected node did not
      change; do not leave earlier root rows clipped off-screen.
- [x] When clearing search, return the tree viewport to the root.
- [x] Verified with a match near the beginning of a large pasted JSON document
      while the tree is scrolled far down.

## Split mode tree while editing

- [x] Mark the tree as out of date as soon as the editor text changes while
      split mode is active.
- [x] Keep showing the last successfully parsed tree when the current text is
      invalid, and make its stale state visible in the status bar
      (`Tree out of date — press Refresh to update`).
- [x] After parsing succeeds, preserve the selected node and expanded branches
      when their IDs still exist. Fall back to the root or collapse missing
      branches when nodes no longer exist.
- [x] Clear the out-of-date state after a successful parse and keep parse
      errors visible in the editor.

## Repeated object keys in the tree

- [x] The parser preserves repeated keys, and the properties panel lists each
      one, while tree nodes receive unique internal IDs separate from JSON
      paths.
- [x] Match macOS/JavaScript semantics: repeated object keys resolve to the
      last value at the key's first position; badges and copied subtrees show
      the resolved object.
- [x] Verified repeated entries do not collapse unrelated nodes and that
      selections resolve through unique IDs.

## Verification

- [x] `cargo check` (no warnings).
- [x] Model/parser unit tests: 6 passed under Wine 8 (repeated keys, unique
      IDs/selection, leaf property grid, background search/cancellation,
      Python literal parsing).
- [x] Wine GUI smoke test: dark theme, tabs, Viewer tree, property grid,
      property header/filter/parent jump, search reveal and status,
      clear-search scroll-to-root, repeated keys, command-line file open.
- [x] Drag-and-drop handler compiled into the drop-target window; exercising
      shell drag-and-drop was not possible in the headless Wine environment.
- [x] Release build passes PE checks: x86-64 GUI subsystem, embedded
      icon/manifest/version info, only system DLL imports.
- [x] ClamAV 1.4.3 scan: clean (exit 0, no detections).

## Remaining external limitation

- [ ] Authenticode code signing. The binary is unsigned, so SmartScreen or
      heuristic AV warnings are possible on first run. Full suppression needs
      a purchased code-signing certificate; see `README.md`.
