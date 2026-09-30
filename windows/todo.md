# Windows follow-up

## Search and Properties navigation

- When Enter, Go, Next, Previous, or a Properties row selects a match, expand its ancestors before scrolling the tree to that node.
- Trigger scrolling after the expanded rows have been rebuilt so virtualized tree rows can be found reliably.
- Verify with a match near the beginning of a large pasted JSON document while the tree is scrolled far down.

## Split mode tree while editing

- Mark the tree as out of date as soon as the editor text changes while split mode is active.
- Keep showing the last successfully parsed tree when the current text is invalid, and make its stale state visible so users know copied tree values may be old.
- After parsing succeeds, preserve the selected node and expanded branches when their JSON paths still exist. Fall back to the root or collapse missing branches when paths no longer exist.
- Clear the out-of-date state after a successful parse and keep parse errors visible in the editor.

## Repeated object keys in the tree

- The parser preserves repeated keys such as `{"count": 3, "count": 23423}`, and the properties panel lists each one, but the tree can hide repeated entries because they currently receive the same node ID.
- Give every tree node a unique, stable internal ID even when multiple object members share the same key or JSON path.
- Verify that all repeated entries appear in the tree and can be selected independently.
