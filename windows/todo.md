# Windows follow-up

## Split mode tree while editing

- Mark the tree as out of date as soon as the editor text changes while split mode is active.
- Keep showing the last successfully parsed tree when the current text is invalid, and make its stale state visible so users know copied tree values may be old.
- After parsing succeeds, preserve the selected node and expanded branches when their JSON paths still exist. Fall back to the root or collapse missing branches when paths no longer exist.
- Clear the out-of-date state after a successful parse and keep parse errors visible in the editor.
