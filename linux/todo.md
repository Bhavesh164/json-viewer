# Linux follow-up

## Search responsiveness

- Run searches on a cancellable background task so clearing or editing the query stays responsive on large JSON documents.
- Traverse nodes with a shared match accumulator to avoid repeatedly copying descendant results.
- Show search-in-progress status and discard results from canceled or outdated queries.

## Search and Properties navigation

- When Enter, Go, Next, Previous, or a Properties row selects a match, expand its ancestors before scrolling the tree to that node.
- Trigger scrolling after the expanded rows have been rebuilt so virtualized tree rows can be found reliably.
- When returning from Text to Viewer with a search match still selected, restore the tree scroll position even if the selected node did not change; do not leave earlier root rows clipped off-screen.
- When clearing search, return the tree viewport to the root.
- Verify with a match near the beginning of a large pasted JSON document while the tree is scrolled far down.

## Split mode tree while editing

- Mark the tree as out of date as soon as the editor text changes while split mode is active.
- Keep showing the last successfully parsed tree when the current text is invalid, and make its stale state visible so users know copied tree values may be old.
- After parsing succeeds, preserve the selected node and expanded branches when their JSON paths still exist. Fall back to the root or collapse missing branches when paths no longer exist.
- Clear the out-of-date state after a successful parse and keep parse errors visible in the editor.

## Repeated object keys in the tree

- A JSON object may repeat a key, and the editor lets you type one, so the tree has to decide what the document means. It shows the value a consumer would actually see: the last occurrence wins, as `JSON.parse` resolves it. Showing every repetition is misleading, because a reader cannot tell which one is in effect.
- Deduplicate in the tree view only. Never rewrite the text: Format, Minify and Save must round-trip the document exactly as the user typed it, including the repeated key.
- A duplicate keeps the position of its first occurrence, so the surrounding keys do not move.
- The properties panel is a view of the same children, so it shows one row per distinct key too.
- Deduplication is per object, not per document: `"a": [{"k":1},{"k":2}]` still shows two `k` rows because they are different elements.
