# JSON Viewer for Linux (Wayland + X11)

Fast, lightweight, native Linux JSON viewer and formatter — the Linux port of
the macOS SwiftUI app in the repository root. Paste or open JSON, explore it
as a hierarchical tree with a property inspector, transform it (format,
minify, stringify, unescape, Python-dict conversion), and search with jump
navigation — all in a single self-contained binary with no installer and no
runtime dependencies beyond system libc.

This is the Linux counterpart of the macOS app (`Sources/`) and the Windows
Win32 port (`windows/`). The core engine (parser, formatter, Python-literal
support, document model, settings) is shared Rust code ported from
`Sources/JSONViewerCore`; the UI is `egui`/`eframe` (Wayland-native via
`winit`, works on Hyprland/Omarchy, Sway, GNOME, KDE, and X11 via XWayland),
the same build pattern as `mouseless/linux` (`cargo build --release` in this
directory, XDG config path).

## Project layout

```
linux/
├── Cargo.toml              # manifest (eframe/egui, rfd, arboard, serde)
├── src/
│   ├── main.rs             # entry point (--help, [FILE], eframe window)
│   ├── app.rs              # egui UI: tree, editor, grid, search, dialogs
│   ├── json.rs             # JSON value model, formatter, order-preserving parser
│   ├── python.rs           # Python dict/literal → JSON parser
│   ├── model.rs            # document model (transforms, search, tree queries)
│   ├── settings.rs         # settings schema + XDG JSON persistence
│   └── clipboard.rs        # Wayland/X11 clipboard (arboard)
└── resources/
    ├── AppIcon.png         # icon (copied from macOS Resources/)
    └── jsonviewer.desktop  # Wayland desktop entry
```

## Features (parity with macOS / Windows)

- **Viewer tab** — hierarchical tree with `key {N}` / `key [N]` container
  badges, `key : value` leaf rows (long text truncated like the mac collapsed
  rows), expand/collapse (all or per node), selection sync, and the selected
  JSON path in the bottom bar. Type-colored rows (string/blue, number/green,
  boolean/yellow, null/red, containers/purple).
- **Expand long text (macOS parity, inline — no popup)** — selecting a long
  string shows it in a full-width panel below the tree with `key • N chars`
  header, Copy, and Collapse. Context menu → *Expand Full Text* jumps to it.
- **Right-click any tree node** — *Expand All Sub-levels*, *Collapse*,
  *Copy Value*, *Copy Key*, *Copy JSON Path*, *Copy Subtree as JSON*,
  *Copy as Python Dictionary*.
- **Text tab** — monospace code editor with live line/character counts and
  valid/invalid status with exact line/column errors. Debounced live re-parse
  (0.6 s) so typing stays smooth on large files.
- **Split tab** — editor on the left with debounced live re-parse, tree on the
  right.
- **Property inspector** — grid showing the selected node's properties
  (`Property / Value / Type`) with in-grid filter; click a property name to
  jump to that node in the tree. Toggle with *Props* (`Ctrl+Alt+P`).
- **Transforms** — Format (2 spaces / 4 spaces / tabs), Minify, Stringify
  (with `\/` escaping option), Unescape, JSON → Python dict, Python → JSON,
  plus Paste with automatic Python-dict-to-JSON conversion and Clear.
- **Multi-format copy** — raw text, beautified, minified, stringified, or
  Python dictionary (Copy dropdown).
- **Search** — query box with Go / Prev / Next, `N of M matches` status,
  Enter/Shift+Enter navigation, Esc to clear; matches auto-expand ancestors
  and reveal the node.
- **File I/O** — Open / Save `.json` via native portal dialogs
  (`Ctrl+O`/`Ctrl+S`). CLI also accepts a file path:
  `jsonviewer data.json`.
- **Settings** — indentation, slash escaping, key sorting, auto-unwrap of
  stringified JSON, default tab, font size (zoom `Ctrl++`/`Ctrl+-`/`Ctrl+0`),
  word wrap. Stored as JSON in `$XDG_CONFIG_HOME/JSONViewer/config.json`
  (`~/.config/JSONViewer/config.json`).
- **Keyboard shortcuts** — `Ctrl+1/2/3` tabs, `Ctrl+E` / `Ctrl+Shift+E`
  expand/collapse all, `Ctrl+Alt+P` properties panel, `Ctrl+F` / `Ctrl+G` /
  `Ctrl+Shift+G` search, `Ctrl+O` / `Ctrl+S` file, `/` quick search,
  `?` shortcuts cheatsheet, `Ctrl+,` settings.

## Prerequisites & System Requirements

A Wayland compositor (Hyprland/Omarchy, Sway, GNOME, KDE) or X11. The binary
is self-contained — it only links system `libc`/`libm`/`libgcc`:

```
linux-vdso.so, libgcc_s.so.1, libm.so.6, libc.so.6
```

### Build dependencies & packages

#### Arch Linux / Omarchy Linux / Manjaro:
```sh
sudo pacman -S base-devel rust wayland libxkbcommon mesa ttf-jetbrains-mono-nerd
```

#### Ubuntu / Debian:
```sh
sudo apt update
sudo apt install build-essential cargo rustc libwayland-dev libxkbcommon-dev libgl1-mesa-dev fonts-dejavu-core
```

#### Fedora:
```sh
sudo dnf install gcc cargo rust wayland-devel libxkbcommon-devel mesa-libGL-devel dejavu-sans-fonts
```

## How to Build & Install Manually

You can build and install either using the provided `Makefile` or standard `cargo` commands:

### 1. Fast Development Build (Quick compile)
Run this when developing and testing quick code edits:
```sh
# From repository root:
make build

# Or directly in linux/:
cd linux && cargo build
```
The binary will be located at:
```
linux/target/debug/jsonviewer
```

### 2. Optimized Release Build (LTO & symbol stripped)
Run this to generate the production binary with Link-Time Optimization and stripped debug symbols (~12 MB):
```sh
# From repository root:
make release

# Or directly in linux/:
cd linux && cargo build --release
```
The binary will be located at:
```
linux/target/release/jsonviewer
```

### 3. Install for Current User (Recommended, No `sudo` needed)
Copies the release binary to your `~/.local/bin` (already on `$PATH` in Omarchy and modern distributions) and self-registers the desktop entry and icons for application launchers:
```sh
cp linux/target/release/jsonviewer ~/.local/bin/jsonviewer
~/.local/bin/jsonviewer --install
```

### 4. Install System-wide into `/usr/local/bin` (Requires `sudo`)
Installs the binary into `/usr/local/bin` and the `.desktop` launcher and icons into system-wide `/usr/share/`:
```sh
# From repository root:
sudo make install

# Or inside linux/:
cd linux && sudo make install
```

### 5. Run Unit Tests
Run the test suite (JSON parsing, Python literals, document model, tree flattening, search, font loading):
```sh
# From repository root:
make test

# Or inside linux/:
cd linux && cargo test
```

## How to Run

```sh
# Run installed binary
jsonviewer [FILE]

# Or run directly from build output
./linux/target/release/jsonviewer [FILE]
```

- `jsonviewer` — launch with the built-in sample document.
- `jsonviewer data.json` — open a file directly.
- `jsonviewer --install` — install/refresh desktop launcher and icons in `~/.local/share/`.
- `jsonviewer --help` — print usage and options.

## Configuration

`$XDG_CONFIG_HOME/JSONViewer/config.json`
(fallback `~/.config/JSONViewer/config.json`). A default file is written on
first run. Example:

```json
{
  "indentSpaces": 2,
  "sortKeysAlphabetically": false,
  "escapeSlashesInStringify": true,
  "autoUnwrapStringified": true,
  "defaultTab": "Text",
  "fontSize": 13.0,
  "wrapLines": true
}
```

## Architecture

- `main.rs` — CLI (`--help`, `[FILE]`), XDG settings load, eframe window.
- `app.rs` — egui UI (toolbar, search, tree, editor, grid, dialogs,
  shortcuts, debounced re-parse).
- `model.rs` — platform-independent document state machine (portable core).
- `json.rs` — order-preserving JSON parser/formatter with line/col errors.
- `python.rs` — Python dict/literal → JSON parser.
- `settings.rs` — XDG config load/save.
- `clipboard.rs` — `arboard` clipboard + `egui::Context::copy_text`.
