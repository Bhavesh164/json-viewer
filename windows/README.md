# JSON Viewer — Windows port

A fast, lightweight, native Windows JSON viewer and formatter with a dark
theme. Paste or open JSON, explore it as a hierarchical tree with a property
inspector, transform it (format, minify, stringify, unescape, Python-dict
conversion), and search with jump navigation — all in a single
self-contained `.exe` with no installer and no runtime dependencies.

This is the Windows counterpart of the macOS SwiftUI app in the repository
root. The core engine (parser, formatter, Python-literal support, document
model, settings) is a faithful Rust port of `Sources/JSONViewerCore`; the UI
is a native Win32 application written with the same stack as
`mouseless/windows` (`windows-rs 0.58`, raw Win32 controls, icon embedded via
`build.rs` + `windres`).

## Project layout

```
windows/
├── Cargo.toml              # manifest (windows-rs 0.58, serde)
├── Cargo.lock
├── build.rs                # compiles resources/icon into the .exe
├── .cargo/config.toml      # default target: x86_64-pc-windows-gnu
├── resources/
│   ├── AppIcon.ico         # multi-size icon (16–256px) embedded in the .exe
│   └── resource.rc         # resource script (icon ID 1 + version info)
└── src/
    ├── main.rs             # entry point (windows_subsystem, message loop)
    ├── app.rs              # main window: editor, tree, grid, search, menus
    ├── json.rs             # JSON value model, formatter, order-preserving parser
    ├── python.rs           # Python dict/literal → JSON parser
    ├── model.rs            # document model (transforms, search, tree queries)
    ├── settings.rs         # settings schema + JSON persistence
    ├── settings_dialog.rs  # settings window (raw Win32 controls)
    ├── clipboard.rs        # Unicode clipboard helpers
    └── util.rs             # UTF-16 / window-text helpers
```

## Features (parity with macOS)

- **Viewer tab** — hierarchical tree (`SysTreeView32`) with `key {N}` / `key [N]`
  container badges, `key : value` leaf rows (long text truncated like the mac
  collapsed rows), expand/collapse (all or per node), selection sync, and the
  selected JSON path in the bottom bar.
- **Expand long text** — double-click a truncated value, or right-click →
  *Expand Full Text…*, to open the complete wrapped text with character count
  and Copy (the Windows equivalent of the mac inline `[expand]` view).
- **Right-click any tree node** — *Expand All Sub-levels*, *Collapse*,
  *Copy Value*, *Copy Key*, *Copy JSON Path*, *Copy Subtree as JSON*,
  *Copy as Python Dictionary*.
- **Progressive loading** — huge files (1 MB+) fill the tree in chunks with a
  `Loading tree… N of M nodes` progress state; the window never freezes.
  Grids cap at 10,000 rows with a status note.
- **Text tab** — monospace editor with word-wrap setting, live line/character
  counts, and valid/invalid status with exact line/column errors.
- **Split tab** — editor on the left with debounced live re-parse, tree on the
  right.
- **Property inspector** — report-style grid showing the selected node's
  properties (`Property / Value / Type`); double-click a row to jump to that
  node in the tree.
- **Transforms** — Format (2 spaces / 4 spaces / tabs), Minify, Stringify
  (with `\/` escaping option), Unescape, JSON → Python dict, Python → JSON,
  plus Paste with automatic Python-dict-to-JSON conversion and Clear.
- **Multi-format copy** — raw text, beautified, minified, stringified, or
  Python dictionary.
- **Search** — query box with GO / Previous / Next, `N of M matches` status,
  Enter/Shift+Enter navigation, Esc to clear; matches auto-expand ancestors
  and reveal the node.
- **File I/O** — Open / Save `.json` via native file dialogs (`Ctrl+O`/`Ctrl+S`).
- **Settings** — indentation, slash escaping, key sorting, auto-unwrap of
  stringified JSON, default tab, font size (zoom `Ctrl++`/`Ctrl+-`/`Ctrl+0`),
  word wrap. Stored as JSON in `%APPDATA%\JSONViewer\config.json`.
- **Keyboard shortcuts** — `Ctrl+1/2/3` tabs, `Ctrl+E` / `Ctrl+Shift+E`
  expand/collapse all, `Ctrl+Alt+P` properties panel, `Ctrl+F` / `Ctrl+G` /
  `Ctrl+Shift+G` search, `Ctrl+X` / `Ctrl+C` / `Ctrl+V` / `Ctrl+A` cut/copy/
  paste/select-all (paste auto-converts Python dicts in the editor, like mac),
  `/` quick search, `?` shortcuts cheatsheet.

## Prerequisites

- **Rust** (stable, 1.75+ recommended)
- The `x86_64-pc-windows-gnu` target:
  ```sh
  rustup target add x86_64-pc-windows-gnu
  ```
- A Windows resource compiler for embedding the icon:
  - **Native Windows (MSVC):** `rc.exe` ships with Visual Studio Build Tools.
  - **Native Windows (GNU):** `windres` ships with MinGW-w64.
  - **Cross-compilation from Linux:** see below.

## Building on Windows (native)

```sh
cd windows
cargo build --release
```

The output binary is at:

```
target/x86_64-pc-windows-gnu/release/jsonviewer.exe
```

`build.rs` automatically finds `windres` (or `rc.exe`), compiles
`resources/resource.rc`, and links the icon + version info into the `.exe`. If
no resource compiler is found the build still succeeds — it just won't have
the embedded icon.

## Cross-compiling from Linux (x86_64)

Because this project uses the `x86_64-pc-windows-gnu` target, you need a
MinGW-w64 toolchain. The easiest self-contained option is
[llvm-mingw](https://github.com/mstorsjo/llvm-mingw):

```sh
# 1. Download & extract llvm-mingw (ucrt, x86_64 host)
curl -sL -o llvm-mingw.tar.xz \
  "https://github.com/mstorsjo/llvm-mingw/releases/download/20260421/llvm-mingw-20260421-ucrt-ubuntu-22.04-x86_64.tar.xz"
tar -xf llvm-mingw.tar.xz

# 2. Put it on PATH
export PATH="$PWD/llvm-mingw-20260421-ucrt-ubuntu-22.04-x86_64/bin:$PATH"

# 3. Shim libgcc (llvm-mingw uses compiler-rt + libunwind instead)
MINGW="$PWD/llvm-mingw-20260421-ucrt-ubuntu-22.04-x86_64"
mkdir -p mingw-stubs
cp "$MINGW/lib/clang/22/lib/windows/libclang_rt.builtins-x86_64.a" mingw-stubs/libgcc.a
cp "$MINGW/x86_64-w64-mingw32/lib/libunwind.a"                  mingw-stubs/libgcc_eh.a

# 4. Tell cargo to use the MinGW linker
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-L $PWD/mingw-stubs"

# 5. Build
cd windows
cargo build --release
```

## Embedding the icon

The app icon (`resources/AppIcon.ico`) is embedded into the `.exe` at build
time via `build.rs` + `resources/resource.rc` (icon resource ID `1`, plus
`VS_VERSION_INFO`). It was generated from the macOS `Resources/AppIcon.png`
(multi-size: 256, 128, 64, 48, 32, 16 px):

```sh
convert Resources/AppIcon.png -define icon:auto-resize=256,128,64,48,32,16 \
  windows/resources/AppIcon.ico
```

## Is the binary self-contained?

**Yes.** The release `.exe` has no external runtime dependencies. It only
imports Windows system DLLs that ship with every Windows 10/11 install:

```
kernel32.dll, user32.dll, gdi32.dll, comctl32.dll, comdlg32.dll,
oleaut32.dll, userenv.dll, ws2_32.dll, ntdll.dll, ...
```

There is **no** dependency on `VCRUNTIME140.dll`, `libgcc_s_seh-1.dll`,
`libwinpthread-1.dll`, or any other redistributable runtime. You can copy the
single `jsonviewer.exe` to any Windows 10/11 machine and run it directly.

## Code signing (SmartScreen)

The binary is **unsigned**. On first launch Windows SmartScreen may show
"Windows protected your PC" — click **More info → Run anyway**. This is normal
for any unsigned application. Permanently suppressing the warning requires a
purchased **Authenticode code-signing certificate**, which is a commercial
purchase, not a build-tooling change.

## Settings

Stored as JSON in `%APPDATA%\JSONViewer\config.json`. Defaults:

| Setting                | Default            |
|------------------------|--------------------|
| Indentation            | 2 spaces           |
| Sort keys              | off                |
| Escape slashes (`\/`)  | on                 |
| Auto-unwrap stringified| on                 |
| Default tab            | Text               |
| Font size              | 12 pt              |
| Wrap lines             | off                |
