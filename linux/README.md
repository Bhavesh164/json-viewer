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
├── install-omarchy.sh      # one-command installer for Omarchy (remove + install)
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
  and reveal the node. Matching runs on a cancellable background thread
  (`Searching…` while it works) so editing or clearing the query stays
  responsive on large documents, and results from canceled or outdated
  queries are discarded. Clearing search returns the tree to its root.
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

## Install on Omarchy (One Command)

`install-omarchy.sh` is the fastest way to get JSON Viewer onto an Omarchy
machine. It removes any previously installed copy, builds the current source,
installs it, and makes sure it appears in the Omarchy application menu.

```sh
cd linux
./install-omarchy.sh
```

That single command:

1. Stops a running JSON Viewer instance (so the binary can be replaced).
2. Removes the previous user-level install — `~/.local/bin/jsonviewer`, the
   `.desktop` entry, and every icon path.
3. Removes a system-wide install at `/usr/local/bin` and `/usr/share` if one is
   found, so it cannot shadow the new copy on `$PATH`.
4. Runs `cargo build --release`.
5. Installs the binary to `~/.local/bin/jsonviewer`.
6. Registers the desktop entry and icons via the app's own `--install` mode.
7. Refreshes the desktop database, icon cache, and the Omarchy menu.
8. Verifies the result and prints what to run next.

Then open the Omarchy menu with <kbd>Super</kbd>+<kbd>Space</kbd> and search for
**JSON Viewer**.

### Options

| Flag | Effect |
| --- | --- |
| *(none)* | Remove previous install, build release, install (default) |
| `--no-build` | Skip the build and install the existing `target/release/jsonviewer` |
| `--run` | Launch the app once installed |
| `--uninstall` | Remove the installation and exit |
| `--help` | Show usage |

Examples:

```sh
# Rebuild and reinstall after editing the source
./install-omarchy.sh

# Reinstall the current binary without recompiling (fast iteration)
./install-omarchy.sh --no-build --run

# Remove it completely
./install-omarchy.sh --uninstall
```

The script is idempotent — re-running it always removes the old copy first and
leaves a single clean install, so there is never a stale duplicate in the menu.

> [!NOTE]
> The `.desktop` entry and icons are written by the app itself
> (`src/desktop.rs`), which also runs automatically on every launch. The script
> calls `jsonviewer --install` rather than writing the entry by hand, so the
> script and the running app can never disagree about the file's contents.
> Your settings in `~/.config/JSONViewer/config.json` are never touched.

### Manual equivalent

If you prefer not to use the script, `jsonviewer --install` (see
[below](#4-install-for-current-user-recommended-no-sudo-needed)) registers the
desktop entry and icons on its own.

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

### 3. Package for Release (`.tar.gz` & standalone binary)
Run this command to compile the release binary and bundle it into a distributable archive ready for GitHub Releases:
```sh
# Inside linux/:
cd linux && make package

# Or from repository root:
make -C linux package
```
This generates three artifacts in `linux/target/release/`:
- **`jsonviewer-linux-x86_64.tar.gz`** (~6 MB) — Recommended release archive containing the self-contained binary `jsonviewer`, `jsonviewer.desktop` entry, and application icons.
- **`jsonviewer-linux-x86_64.zip`** — Same contents as the `.tar.gz` archive, for systems without `tar` (needs the `zip` command; skipped with a warning when unavailable).
- **`jsonviewer-linux-x86_64`** (~11.8 MB) — Standalone single executable (copy and run directly).

> [!NOTE]
> **Zero External Dependencies**: The Linux binary is 100% self-contained and only links to system `libc`/`libm`/`libgcc`. All fonts (JetBrains Mono) and icons are embedded directly into the binary at compile time. It runs out-of-the-box on Ubuntu, Debian, Fedora, Arch Linux, Manjaro, openSUSE, and any standard Linux distribution without installing extra packages.

### 4. Install for Current User (Recommended, No `sudo` needed)
Copies the release binary to your `~/.local/bin` (already on `$PATH` in Omarchy and modern distributions) and self-registers the desktop entry and icons for application launchers:
```sh
cp linux/target/release/jsonviewer ~/.local/bin/jsonviewer
~/.local/bin/jsonviewer --install
```

> [!TIP]
> On Omarchy, prefer [`./install-omarchy.sh`](#install-on-omarchy-one-command) —
> it does the above *and* removes any previous install first, so you never end up
> with a stale duplicate in the application menu.

### 5. Install System-wide into `/usr/local/bin` (Requires `sudo`)
Installs the binary into `/usr/local/bin` and the `.desktop` launcher and icons into system-wide `/usr/share/`:
```sh
# From repository root:
sudo make install

# Or inside linux/:
cd linux && sudo make install
```

### 6. Run Unit Tests
Run the test suite (JSON parsing, Python literals, document model, tree flattening, search, font loading):
```sh
# From repository root:
make test

# Or inside linux/:
cd linux && cargo test
```

## 📦 What to Upload to GitHub Releases

When publishing a new release on GitHub, upload these files from `linux/target/release/`:
1. **`jsonviewer-linux-x86_64.tar.gz`** (Primary download)
2. **`jsonviewer-linux-x86_64.zip`** (Alternative archive)
3. **`jsonviewer-linux-x86_64`** (Optional standalone single-binary download)

### How End-Users Run It on Any Linux Machine

No installation or dependencies required:
```sh
# Extract and launch (tar.gz or zip)
tar -xzf jsonviewer-linux-x86_64.tar.gz
unzip jsonviewer-linux-x86_64.zip
./jsonviewer

# (Optional) Register desktop launcher in system app menus
./jsonviewer --install
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
