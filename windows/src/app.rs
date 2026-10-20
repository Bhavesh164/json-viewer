//! Main Win32 window for JSON Viewer.
//! Native controls mirroring the macOS app: text editor (EDIT), hierarchical
//! tree (SysTreeView32), property grid (SysListView32), search bar, status bar,
//! menus + toolbar + accelerators, settings / shortcuts / about dialogs.

use crate::clipboard;
use crate::json::JSONParser;
use crate::model::{flatten, AppTab, DocumentModel, FlatRow, ParentInfo, PropertyRow};
use crate::python::parse_python_literal;
use crate::settings::{Settings, SettingsStore};
use crate::settings_dialog::{self, SettingsContext};
use crate::util::{get_edit_text, get_window_text, set_editor_text, set_window_text, wide};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{BOOL, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, DrawFocusRect, DrawTextW, FillRect,
    GetDeviceCaps, ReleaseDC, GetDC, GetTextExtentPoint32W, InvalidateRect, ScreenToClient,
    SelectObject, SetBkColor, SetBkMode,
    SetTextColor, HFONT, LOGPIXELSY,
    DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY, DEFAULT_PITCH,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_NORMAL, OPAQUE, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, INITCOMMONCONTROLSEX, DRAWITEMSTRUCT, EM_REPLACESEL, EM_SETSEL,
    ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES,
    ICC_TREEVIEW_CLASSES, LVCOLUMNW, LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT, LVCF_WIDTH, LVCFMT_LEFT,
    LVIF_TEXT,     LVIS_SELECTED, LVITEMW, LVM_DELETEALLITEMS, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW,
    LVM_INSERTITEMW, LVM_SETBKCOLOR, LVM_SETCOLUMNWIDTH, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMSTATE,
    LVM_SETITEMTEXTW, LVM_SETTEXTCOLOR, LVM_SETTEXTBKCOLOR, LVNI_SELECTED,
    LVS_EX_FULLROWSELECT, LVS_EX_GRIDLINES, LVS_REPORT, LVS_SHOWSELALWAYS, LVS_SINGLESEL,
    ODS_DISABLED, ODS_FOCUS, ODS_SELECTED, SetWindowTheme,
    TVHITTESTINFO, TVHT_ONITEM, TVINSERTSTRUCTW, TVM_DELETEITEM, TVM_ENSUREVISIBLE, TVM_EXPAND,
    TVM_GETNEXTITEM, TVM_HITTEST, TVM_INSERTITEMW, TVM_SELECTITEM, TVM_SETBKCOLOR,
    TVM_SETLINECOLOR, TVM_SETTEXTCOLOR, TVE_COLLAPSE, TVE_EXPAND, TVGN_CARET, TVGN_CHILD,
    TVGN_FIRSTVISIBLE, TVGN_NEXT, TVIF_TEXT, TVI_LAST, TVI_ROOT,
    TVS_DISABLEDRAGDROP, TVS_HASBUTTONS, TVS_HASLINES,
    TVS_LINESATROOT, TVS_SHOWSELALWAYS, WC_LISTVIEW, WC_TREEVIEW, LVN_ITEMCHANGED, NM_CLICK,
    TVN_SELCHANGEDW, TVN_ITEMEXPANDEDW, NMTREEVIEWW, NMITEMACTIVATE, NM_CUSTOMDRAW, NMTVCUSTOMDRAW,
    NMLVCUSTOMDRAW, CDDS_PREPAINT, CDDS_ITEMPREPAINT, CDDS_SUBITEM,
    CDRF_DODEFAULT, CDRF_NEWFONT, CDRF_NOTIFYITEMDRAW, CDRF_NOTIFYSUBITEMDRAW,
    CDIS_SELECTED,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
    OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
};
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, GetKeyState, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    ACCEL, AppendMenuW,BN_CLICKED, CallWindowProcW, CreateAcceleratorTableW, CreateMenu,
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    DrawMenuBar, EN_CHANGE, FALT, FCONTROL, FSHIFT, FVIRTKEY, GetCursorPos, GetMessageW,
    GetWindowLongPtrW, GetWindowRect, SetForegroundWindow,
    GWLP_USERDATA, GWLP_WNDPROC, HMENU, LoadCursorW, LoadIconW, MessageBoxW,
    PostMessageW, PostQuitMessage, RegisterClassW, SendMessageW, SetMenu,
    SetWindowLongPtrW,
    SetWindowPos, ShowWindow, SW_HIDE, SW_SHOW, SWP_NOZORDER, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
    TPM_TOPALIGN, TrackPopupMenu, TranslateAcceleratorW, TranslateMessage,
    BS_OWNERDRAW, IDC_ARROW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MF_POPUP, MF_SEPARATOR,
    MF_STRING, MINMAXINFO, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW, WNDPROC, WS_BORDER,
    WS_CAPTION, WS_CLIPCHILDREN, WS_HSCROLL, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
    WS_OVERLAPPEDWINDOW, WS_SYSMENU, WS_TABSTOP, WS_VSCROLL, ES_AUTOHSCROLL,
    ES_AUTOVSCROLL, ES_MULTILINE, ES_WANTRETURN, ES_READONLY, WM_APP, WM_COMMAND, WM_CONTEXTMENU, WM_CREATE,
    WM_COPY, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_CUT, WM_DESTROY, WM_DRAWITEM, WM_NOTIFY,
    WM_PASTE, WM_SETFONT, WM_SIZE, WM_TIMER, WM_USER, WM_CHAR, WM_DROPFILES,
    WM_CLOSE, WM_NCDESTROY, WM_KEYDOWN, WM_SYSKEYDOWN,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};

// Control IDs — Row 1: tab switcher (left) + utilities (right)
const IDC_TAB_VIEWER: u32 = 101;
const IDC_TAB_TEXT: u32 = 102;
const IDC_TAB_SPLIT: u32 = 103;
const IDC_BTN_PROPS: u32 = 104;
const IDC_BTN_FIND: u32 = 105;
const IDC_BTN_SHORTCUTS: u32 = 106;
const IDC_BTN_SETTINGS: u32 = 107;
// Row 2: contextual actions (mirrors the macOS toolbars)
const IDC_BTN_FORMAT: u32 = 110;
const IDC_BTN_MINIFY: u32 = 111;
const IDC_BTN_STRINGIFY: u32 = 112;
const IDC_BTN_UNESCAPE: u32 = 113;
const IDC_BTN_J2P: u32 = 114;
const IDC_BTN_P2J: u32 = 115;
const IDC_BTN_COPY: u32 = 116;
const IDC_BTN_PASTE: u32 = 117;
const IDC_BTN_CLEAR: u32 = 118;
const IDC_BTN_EXPAND: u32 = 119;
const IDC_BTN_COLLAPSE: u32 = 120;
const IDC_BTN_OPEN: u32 = 121;
const IDC_BTN_SAVE: u32 = 122;
const IDC_BTN_PARSE: u32 = 123;
const IDC_EDITOR: u32 = 200;
const IDC_TREE: u32 = 201;
const IDC_GRID: u32 = 202;
const IDC_SEARCH_EDIT: u32 = 211;
const IDC_SEARCH_GO: u32 = 212;
const IDC_SEARCH_PREV: u32 = 213;
const IDC_SEARCH_NEXT: u32 = 214;
const IDC_SEARCH_STATUS: u32 = 215;
const IDC_PROPS_TITLE: u32 = 216;
const IDC_PROPS_PARENT: u32 = 217;
const IDC_PROPS_FILTER: u32 = 218;
const IDC_PROPS_COUNT: u32 = 219;
const IDC_PROPS_FILTER_LABEL: u32 = 222;
const IDC_STATUS: u32 = 220;
const IDC_PATH: u32 = 221;
// Inline expand panel (macOS parity: full-width wrapped text below the tree,
// not a separate popup). Docked inside the main window.
const IDC_EXPAND_LABEL: u32 = 230;
const IDC_EXPAND_TEXT: u32 = 231;
const IDC_EXPAND_COPY: u32 = 232;
const IDC_EXPAND_COLLAPSE: u32 = 233;

// Menu-only command IDs
const IDM_EXIT: u32 = 1003;
const IDM_COPY_TEXT: u32 = 1100;
const IDM_COPY_BEAUTIFIED: u32 = 1104;
const IDM_COPY_MINIFIED: u32 = 1105;
const IDM_COPY_STRINGIFIED: u32 = 1106;
const IDM_COPY_PYTHON: u32 = 1107;
const IDM_TOGGLE_PROPS: u32 = 1204;
const IDM_ZOOM_IN: u32 = 1205;
const IDM_ZOOM_OUT: u32 = 1206;
const IDM_ZOOM_RESET: u32 = 1207;
const IDM_FIND: u32 = 1301;
const IDM_FIND_NEXT: u32 = 1302;
const IDM_FIND_PREV: u32 = 1303;
const IDM_SETTINGS: u32 = 1401;
const IDM_SHORTCUTS: u32 = 1402;
const IDM_ABOUT: u32 = 1403;

// Global clipboard accelerators (focus-aware: edit controls keep native
// behavior, elsewhere they drive the app).
const IDM_ACCEL_COPY: u32 = 1501;
const IDM_ACCEL_PASTE: u32 = 1502;
const IDM_ACCEL_CUT: u32 = 1503;
const IDM_ACCEL_SELECTALL: u32 = 1504;

// Tree context-menu commands.
const IDM_CTX_EXPAND_SUB: u32 = 1601;
const IDM_CTX_COLLAPSE: u32 = 1602;
const IDM_CTX_COPY_VALUE: u32 = 1603;
const IDM_CTX_COPY_KEY: u32 = 1604;
const IDM_CTX_COPY_PATH: u32 = 1605;
const IDM_CTX_COPY_SUBTREE: u32 = 1606;
const IDM_CTX_COPY_PYTHON: u32 = 1607;
const IDM_CTX_EXPAND_TEXT: u32 = 1608;

const WM_APP_SETTINGS_CHANGED: u32 = WM_USER + 101;
const TIMER_LIVE_PARSE: usize = 1;
const TIMER_FILL: usize = 2;
/// Polls the background search worker and applies its results.
const TIMER_SEARCH: usize = 3;
/// Tree rows per timer tick — keeps the UI responsive while huge files load.
/// Each tick is wrapped in WM_SETREDRAW off/on (see `fill_tick`) so 4000
/// inserts cost one repaint instead of 4000. Benchmarks: 204k nodes from a
/// ~5MB file fill in ~50 ticks without freezing typing.
const FILL_CHUNK: usize = 4000;
/// Max property-grid rows (ListView is not virtualized; beyond this we show
/// a note instead of freezing).
const GRID_CAP: usize = 10_000;
/// Above this size the Split tab stops auto-reparsing on every idle tick —
/// parsing + flattening 1MB+ on the UI thread (≈300ms for 5MB) is what made
/// typing feel "stuck". The tree updates on tab switch / Refresh instead.
const LARGE_FILE_BYTES: usize = 1_000_000;
/// EM_SETLIMITTEXT (0x00C5): default EDIT limit is 32KB of typed text.
/// Without raising it, large pastes/types beyond 32KB are silently blocked.
const EM_SETLIMITTEXT: u32 = 0x00C5;
/// EM_SETCUEBANNER (comctl32 v6): placeholder text in an empty EDIT control.
const EM_SETCUEBANNER: u32 = 0x1501;
/// Max editor text (64MB UTF-16 units) — well above 5MB test files.
const EDIT_TEXT_LIMIT: usize = 64 * 1024 * 1024;
/// STATIC style: vertically center single-line text in its rect, so labels
/// sit on the same baseline as adjacent buttons/edits (SS_LEFT stays 0).
const SS_CENTERIMAGE: u32 = 0x0200;

// Dark theme (default, like the mac app's dark appearance).
const DARK_EDIT_BG: u32 = 0x1E1E1E;
const DARK_CHROME_BG: u32 = 0x252526;
const DARK_TEXT: u32 = 0xD4D4D4;
const DARK_DIM_TEXT: u32 = 0x858585;
const DARK_BORDER: u32 = 0x3F3F46;
const DARK_BTN: u32 = 0x2D2D2D;
const DARK_BTN_DOWN: u32 = 0x00714709; // #094771 in COLORREF byte order
const DARK_ACCENT_BG: u32 = 0x00714709;
// Search matches (non-current): bright yellow text on the dark row, like the
// mac yellow MATCH badge — distinct from the brown selection background.
const SEARCH_MATCH_TEXT: u32 = 0x0000FFFF; // COLORREF 0x00BBGGRR: R=FF G=FF B=00

/// Per-type row/value colors (VS Code dark palette, converted to COLORREF's
/// 0x00BBGGRR byte order). Makes the tree and property grid scannable at a
/// glance without relying on the Type column alone.
fn type_color(type_name: &str) -> u32 {
    match type_name {
        "string" => 0x007891CE,  // #CE9178
        "number" => 0x00A8CEB5,  // #B5CEA8
        "boolean" => 0x00D69C56, // #569CD6
        "null" => 0x00808080,    // #808080
        "array" => 0x00AAAADC,   // #DCDCAA
        "object" => 0x00B0C94E,  // #4EC9B0
        _ => DARK_TEXT,
    }
}

static mut APP_PTR: usize = 0;
/// Original search-edit proc. Global (not via `app_of`) so the subclass can
/// forward pass-through messages WITHOUT borrowing `App` — borrowing it here
/// while `wnd_proc` already holds `&mut App` (e.g. `WM_GETTEXT` during
/// `EN_CHANGE`) would be aliased-mutable UB.
static SEARCH_ORIG_PROC: std::sync::atomic::AtomicIsize =
    std::sync::atomic::AtomicIsize::new(0);

fn send(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize {
    unsafe { SendMessageW(hwnd, msg, WPARAM(w), LPARAM(l)).0 }
}

const WM_SETREDRAW: u32 = 0x000B;

/// Freeze/thaw painting around bulk control updates (tree/grid fills with
/// thousands of nodes). Without this every insert repaints and large files
/// take seconds instead of milliseconds.
fn set_redraw(hwnd: HWND, on: bool) {
    send(hwnd, WM_SETREDRAW, if on { 1 } else { 0 }, 0);
    if on {
        unsafe {
            let _ = InvalidateRect(hwnd, None, BOOL(1));
        }
    }
}

fn load_app_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let name = windows::core::PCWSTR(1usize as *const u16);
        LoadIconW(instance, name).unwrap_or_default()
    }
}

/// One button in a toolbar bar. A spec with `id == 0` is pure spacing
/// (no control at all) — group dividers are gaps, never box-looking widgets.
struct BarItem {
    id: u32,
    hwnd: HWND,
    width: i32,
}

struct Controls {
    // Row 1: tab switcher (left) + utilities (right) — like the macOS toolbar.
    row_tabs: Vec<BarItem>,
    row_utils: Vec<BarItem>,
    // Row 2: contextual actions per tab — like the macOS Text/Tree toolbars.
    row_text: Vec<BarItem>,
    row_viewer: Vec<BarItem>,
    editor: HWND,
    tree: HWND,
    grid: HWND,
    // Inline expand panel: full-width wrapped text below the tree
    // (macOS expanded big-text parity — no separate popup window).
    expand_label: HWND,
    expand_text: HWND,
    expand_copy: HWND,
    expand_collapse: HWND,
    search_label: HWND,
    search_edit: HWND,
    search_go: HWND,
    search_prev: HWND,
    search_next: HWND,
    search_status: HWND,
    status: HWND,
    path: HWND,
    // Property inspector header: title + parent jump + in-grid filter.
    prop_title: HWND,
    prop_parent: HWND,
    prop_filter_label: HWND,
    prop_filter: HWND,
    prop_count: HWND,
}

struct CreateParams {
    settings: Arc<Mutex<Settings>>,
    store: Arc<Mutex<SettingsStore>>,
    initial_file: Option<std::path::PathBuf>,
}

struct App {
    hwnd: HWND,
    settings: Arc<Mutex<Settings>>,
    store: Arc<Mutex<SettingsStore>>,
    model: DocumentModel,
    ctrl: Controls,
    font_mono: HFONT,
    font_ui: HFONT,
    brush_edit: windows::Win32::Graphics::Gdi::HBRUSH,
    brush_chrome: windows::Win32::Graphics::Gdi::HBRUSH,
    item_to_id: HashMap<isize, String>,
    id_to_item: HashMap<String, isize>,
    /// Row colours for the native TreeView custom draw.
    item_type: HashMap<isize, &'static str>,
    container_items: Vec<isize>,
    /// Container IDs expanded in the native tree. Captured on
    /// TVN_ITEMEXPANDED and re-applied after a tree rebuild so Split-mode
    /// parsing does not collapse the user's branches.
    expanded_ids: HashSet<String>,
    grid_rows: Vec<PropertyRow>,
    grid_parent: Option<ParentInfo>,
    grid_truncated: Option<usize>,
    grid_total: usize,
    prop_filter: String,
    /// True while we programmatically rewrite the filter edit, so its
    /// EN_CHANGE does not rebuild the grid again.
    syncing_filter: bool,
    syncing_editor: bool,
    /// True while we (not the user) change grid selection — stops the
    /// resulting LVN_ITEMCHANGED from re-entering the click handler and
    /// rebuilding the grid in an infinite loop.
    syncing_grid: bool,
    props_visible: bool,
    search_orig_proc: isize,
    /// In-progress chunked tree fill (None when idle).
    fill: Option<TreeFill>,
    fill_gen: u64,
    /// True when the TreeView fully matches the current model (no partial
    /// fill from a cancelled load). Needed so tab switches don't leave a
    /// half-filled tree.
    tree_complete: bool,
    /// True while the editor text differs from the tree currently shown (or
    /// the latest parse failed). Split mode keeps the last good tree visible
    /// and surfaces this as a status-bar warning so copied values are not
    /// mistaken for fresh ones.
    tree_stale: bool,
    /// Expand-all requested while a fill is running — applied on completion.
    expand_on_fill_done: bool,
    /// Tree node ID right-clicked for the context menu.
    ctx_id: Option<String>,
    /// Inline expand panel state (macOS expanded big-text parity).
    /// `expand_visible == false` means collapsed; `expand_id` is the leaf
    /// whose full text the panel shows (== selected node when visible).
    expand_visible: bool,
    expand_id: Option<String>,
    /// All current search-match IDs (mirrors `model.search_results` for
    /// O(1) custom-draw lookup). Painted yellow; the current match gets the
    /// brown selection background like a manual click.
    search_marks: HashSet<String>,
}

/// One chunked background fill of the native TreeView.
struct TreeFill {
    rows: Vec<FlatRow>,
    next: usize,
    stack: Vec<(usize, isize)>,
    gen: u64,
}

fn app_of(hwnd: HWND) -> Option<&'static mut App> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
        if ptr.is_null() {
            None
        } else {
            Some(&mut *ptr)
        }
    }
}

// ---------------- fonts ----------------

fn make_font(face_name: &str, point_size: f64) -> HFONT {
    unsafe {
        let hdc = GetDC(HWND(std::ptr::null_mut()));
        let dpi = if hdc.0.is_null() { 96 } else { GetDeviceCaps(hdc, LOGPIXELSY) };
        if !hdc.0.is_null() {
            ReleaseDC(HWND(std::ptr::null_mut()), hdc);
        }
        let height = -((point_size * dpi as f64) / 72.0).round() as i32;
        let face = wide(face_name);
        CreateFontW(
            height,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            DEFAULT_QUALITY.0 as u32,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            windows::core::PCWSTR::from_raw(face.as_ptr()),
        )
    }
}

/// Pixel width of `text` in `font`, plus button padding — so toolbar buttons
/// always fit their labels (no clipped "tringif"/"ollaps").
fn measure_button_width(ref_hwnd: HWND, font: HFONT, text: &str) -> i32 {
    unsafe {
        let hdc = GetDC(ref_hwnd);
        if hdc.0.is_null() {
            return (text.len() as i32) * 8 + 24;
        }
        let units: Vec<u16> = text.encode_utf16().collect();
        let mut size = SIZE::default();
        let prev = SelectObject(hdc, font);
        let _ = GetTextExtentPoint32W(hdc, &units, &mut size);
        SelectObject(hdc, prev);
        ReleaseDC(ref_hwnd, hdc);
        (size.cx + 24).max(52)
    }
}

fn set_font(hwnd: HWND, font: HFONT) {
    unsafe {
        if !hwnd.is_invalid() {
            let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
        }
    }
}

/// Dark content views: explorer-style dark theme (dark scrollbars, selection,
/// header) plus explicit text/background/line colors.
fn apply_dark_theme_to_views(app: &App) {
    unsafe {
        let dark = wide("DarkMode_Explorer");
        let sub = windows::core::PCWSTR::from_raw(dark.as_ptr());
        let _ = SetWindowTheme(app.ctrl.tree, sub, windows::core::PCWSTR::null());
        let _ = SetWindowTheme(app.ctrl.grid, sub, windows::core::PCWSTR::null());
        send(app.ctrl.tree, TVM_SETTEXTCOLOR, 0, COLORREF(DARK_TEXT).0 as isize);
        send(app.ctrl.tree, TVM_SETBKCOLOR, 0, COLORREF(DARK_EDIT_BG).0 as isize);
        send(app.ctrl.tree, TVM_SETLINECOLOR, 0, COLORREF(DARK_BORDER).0 as isize);
        send(app.ctrl.grid, LVM_SETTEXTCOLOR, 0, COLORREF(DARK_TEXT).0 as isize);
        send(app.ctrl.grid, LVM_SETTEXTBKCOLOR, 0, COLORREF(DARK_EDIT_BG).0 as isize);
        send(app.ctrl.grid, LVM_SETBKCOLOR, 0, COLORREF(DARK_EDIT_BG).0 as isize);
    }
}

fn apply_font_to_controls(app: &App) {
    // Content views use the zoomable monospace font (like the mac editor/tree).
    for h in [
        app.ctrl.editor,
        app.ctrl.tree,
        app.ctrl.grid,
        app.ctrl.search_edit,
        app.ctrl.search_status,
        app.ctrl.status,
        app.ctrl.path,
        app.ctrl.expand_text,
    ] {
        set_font(h, app.font_mono);
    }
    // Chrome (bars, buttons, labels) uses the native UI font.
    for bar in [&app.ctrl.row_tabs, &app.ctrl.row_utils, &app.ctrl.row_text, &app.ctrl.row_viewer] {
        for item in bar.iter() {
            set_font(item.hwnd, app.font_ui);
        }
    }
    for h in [
        app.ctrl.search_label,
        app.ctrl.search_go,
        app.ctrl.search_prev,
        app.ctrl.search_next,
        app.ctrl.expand_label,
        app.ctrl.expand_copy,
        app.ctrl.expand_collapse,
        app.ctrl.prop_parent,
        app.ctrl.prop_filter_label,
        app.ctrl.prop_filter,
        app.ctrl.prop_title,
        app.ctrl.prop_count,
    ] {
        set_font(h, app.font_ui);
    }
}

// ---------------- control creation ----------------

unsafe fn create_child(
    parent: HWND,
    class_name: &windows::core::PCWSTR,
    text: &str,
    id: u32,
    style: u32,
    ex: u32,
    instance: HINSTANCE,
) -> HWND {
    let txt = wide(text);
    CreateWindowExW(
        WINDOW_EX_STYLE(ex),
        *class_name,
        windows::core::PCWSTR::from_raw(txt.as_ptr()),
        WINDOW_STYLE(style),
        0,
        0,
        10,
        10,
        parent,
        HMENU(id as usize as *mut std::ffi::c_void),
        instance,
        None,
    )
    .unwrap_or(HWND(std::ptr::null_mut()))
}

// Leak tiny static class-name buffers (process-lifetime, negligible).
fn wide_static(s: &[u8]) -> &'static [u16] {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Vec<([u8; 16], Vec<u16>)>> = OnceLock::new();
    let _ = CACHE.get_or_init(Vec::new);
    // Simple leak-based interning without global mutation races: leak each call.
    // Class names are few; duplicates harmless.
    let wide: Vec<u16> = s.iter().take_while(|&&c| c != 0).map(|&c| c as u16).chain(std::iter::once(0)).collect();
    Box::leak(wide.into_boxed_slice())
}

unsafe fn button_class() -> windows::core::PCWSTR {
    windows::core::PCWSTR::from_raw(wide_static(b"BUTTON\0").as_ptr())
}
unsafe fn edit_class() -> windows::core::PCWSTR {
    windows::core::PCWSTR::from_raw(wide_static(b"EDIT\0").as_ptr())
}
unsafe fn static_cls() -> windows::core::PCWSTR {
    windows::core::PCWSTR::from_raw(wide_static(b"STATIC\0").as_ptr())
}

const WS_CHILD_VISIBLE: u32 = 0x40000000 | 0x10000000; // WS_CHILD | WS_VISIBLE

unsafe fn make_button(parent: HWND, text: &str, id: u32, instance: HINSTANCE) -> HWND {
    // Owner-drawn (see WM_DRAWITEM): required for the dark theme, since
    // themed push-buttons cannot be recolored.
    create_child(parent, &button_class(), text, id,
        WS_CHILD_VISIBLE | BS_OWNERDRAW as u32 | WS_TABSTOP.0, 0, instance)
}

unsafe fn make_static(parent: HWND, text: &str, id: u32, instance: HINSTANCE, extra: u32) -> HWND {
    create_child(parent, &static_cls(), text, id, WS_CHILD_VISIBLE | extra, 0, instance)
}

/// Build one toolbar bar from `(id, label)` specs. A spec with `id == 0`
/// is pure spacing between groups (no control is created, so nothing can
/// ever render as a stray box). Button widths are measured with the UI font
/// so labels never clip.
unsafe fn build_bar(
    parent: HWND,
    specs: &[(u32, &str)],
    ui_font: HFONT,
    instance: HINSTANCE,
) -> Vec<BarItem> {
    let mut items = Vec::with_capacity(specs.len());
    for (id, label) in specs {
        if *id == 0 {
            items.push(BarItem { id: 0, hwnd: HWND(std::ptr::null_mut()), width: 14 });
        } else {
            let hwnd = make_button(parent, label, *id, instance);
            set_font(hwnd, ui_font);
            let width = measure_button_width(parent, ui_font, label);
            items.push(BarItem { id: *id, hwnd, width });
        }
    }
    items
}

/// Position a left-aligned bar; returns the x of its right edge.
/// Hidden bars are also zero-sized, so they can never peek through or
/// overlap the visible bar even if Windows re-shows a control.
fn layout_bar(items: &[BarItem], x0: i32, y: i32, h: i32, visible: bool) -> i32 {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE as HIDE};
        let mut x = x0;
        for item in items {
            if item.hwnd.is_invalid() {
                x += item.width + 4;
                continue;
            }
            if visible {
                let _ = SetWindowPos(item.hwnd, HWND(std::ptr::null_mut()), x, y, item.width, h, SWP_NOZORDER);
                let _ = ShowWindow(item.hwnd, SW_SHOW);
            } else {
                let _ = SetWindowPos(item.hwnd, HWND(std::ptr::null_mut()), 0, 0, 0, 0, SWP_NOZORDER);
                let _ = ShowWindow(item.hwnd, HIDE);
            }
            x += item.width + 4;
        }
        x
    }
}

/// Paint an owner-drawn dark toolbar button (WM_DRAWITEM). The active tab
/// gets the accent background, like the mac segmented control.
fn draw_button(app: &App, di: &DRAWITEMSTRUCT) {
    unsafe {
        let selected = (di.itemState.0 & ODS_SELECTED.0) != 0;
        let focus = (di.itemState.0 & ODS_FOCUS.0) != 0;
        let disabled = (di.itemState.0 & ODS_DISABLED.0) != 0;
        let is_active_tab = match app.model.active_tab {
            AppTab::Viewer => di.CtlID == IDC_TAB_VIEWER,
            AppTab::Text => di.CtlID == IDC_TAB_TEXT,
            AppTab::Split => di.CtlID == IDC_TAB_SPLIT,
        };
        let bg = if disabled {
            DARK_BTN
        } else if selected {
            DARK_BTN_DOWN
        } else if is_active_tab {
            DARK_ACCENT_BG
        } else {
            DARK_BTN
        };
        let bg_brush = CreateSolidBrush(COLORREF(bg));
        let border_brush = CreateSolidBrush(COLORREF(DARK_BORDER));
        // 1px border: fill all, then inset.
        FillRect(di.hDC, &di.rcItem, border_brush);
        let mut rc = di.rcItem;
        rc.left += 1;
        rc.top += 1;
        rc.right -= 1;
        rc.bottom -= 1;
        FillRect(di.hDC, &rc, bg_brush);
        let _ = DeleteObject(border_brush);
        let _ = DeleteObject(bg_brush);

        let text = get_window_text(di.hwndItem);
        if !text.is_empty() {
            let units: Vec<u16> = text.encode_utf16().collect();
            let mut buf = units;
            let prev = SelectObject(di.hDC, app.font_ui);
            SetTextColor(di.hDC, COLORREF(if disabled { DARK_DIM_TEXT } else { DARK_TEXT }));
            SetBkMode(di.hDC, TRANSPARENT);
            let mut trc = rc;
            trc.left += 6;
            trc.right -= 6;
            let _ = DrawTextW(
                di.hDC,
                &mut buf,
                &mut trc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(di.hDC, prev);
        }
        if focus {
            let _ = DrawFocusRect(di.hDC, &rc);
        }
    }
}

// ---------------- tree / grid ----------------

fn tree_insert_raw(tree: HWND, parent: isize, after: isize, text: &str) -> isize {
    let w = wide(text);
    let mut item = TVINSERTSTRUCTW::default();
    item.hParent = windows::Win32::UI::Controls::HTREEITEM(parent);
    item.hInsertAfter = windows::Win32::UI::Controls::HTREEITEM(after);
    item.Anonymous.itemex.mask = TVIF_TEXT;
    item.Anonymous.itemex.pszText = windows::core::PWSTR(w.as_ptr() as *mut u16);
    send(tree, TVM_INSERTITEMW, 0, &item as *const _ as isize)
}

fn tree_expand_by_hwnd(tree: HWND, item: isize, expand: bool) {
    send(tree, TVM_EXPAND, if expand { TVE_EXPAND.0 as usize } else { TVE_COLLAPSE.0 as usize }, item);
}

fn tree_expand(app: &App, item: isize, expand: bool) {
    tree_expand_by_hwnd(app.ctrl.tree, item, expand);
}

/// Start filling the native TreeView from the parsed model, in small chunks
/// on a timer so the UI never freezes — even with tens of thousands of
/// nodes the window stays responsive and shows progress. Small trees fill
/// synchronously and feel instant.
///
/// `preserve_nav` keeps the user's expanded branches and selection. It is true
/// for Split-mode re-parses and false for Open / Clear / a new document.
fn begin_tree_fill(app: &mut App, preserve_nav: bool) {
    cancel_tree_fill(app);
    // A fresh tree supersedes any background search started against the old
    // one (the model already cancelled it during parsing).
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(app.hwnd, TIMER_SEARCH);
    }
    app.tree_stale = false;
    app.tree_complete = false;
    if !preserve_nav {
        app.expanded_ids.clear();
        app.prop_filter.clear();
        app.syncing_filter = true;
        unsafe { set_window_text(app.ctrl.prop_filter, ""); }
        app.syncing_filter = false;
    }
    // New tree → stale inline-expand selection is meaningless; hide panel.
    app.expand_visible = false;
    app.expand_id = None;
    send(app.ctrl.tree, TVM_DELETEITEM, 0, TVI_ROOT.0 as isize);
    app.item_to_id.clear();
    app.id_to_item.clear();
    app.item_type.clear();
    app.container_items.clear();
    // Keep search highlights consistent with the model (Clear empties them;
    // re-parse of the same doc preserves them for incoming rows).
    app.search_marks.clear();
    app.search_marks
        .extend(app.model.search_results.iter().cloned());
    send(app.ctrl.grid, LVM_DELETEALLITEMS, 0, 0);
    app.grid_rows.clear();
    app.grid_truncated = None;
    app.grid_total = 0;
    let root = match app.model.root.as_ref() {
        Some(r) => r,
        None => {
            // Empty document is a complete (empty) tree.
            app.tree_complete = true;
            refresh_status(app);
            return;
        }
    };
    // Flatten first (fast, no Win32 calls), then insert in slices.
    let rows = flatten(root.as_ref());
    let total = rows.len();
    if total == 0 {
        app.tree_complete = true;
        refresh_status(app);
        return;
    }
    app.fill = Some(TreeFill { rows, next: 0, stack: Vec::new(), gen: app.fill_gen });
    if total <= FILL_CHUNK {
        // Small enough: do it all now without flicker or timer overhead.
        while !fill_tick(app) {}
        return;
    }
    unsafe {
        set_window_text(app.ctrl.status, &format!("Loading tree… 0 of {} nodes", total));
        let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(app.hwnd, TIMER_FILL, 20, None);
    }
}

fn cancel_tree_fill(app: &mut App) {
    app.fill_gen += 1;
    // Cancelling mid-fill leaves a partial tree — never treat it as complete.
    // (No-op cancels of an already-complete tree keep the flag.)
    if app.fill.is_some() {
        app.tree_complete = false;
    }
    app.fill = None;
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(app.hwnd, TIMER_FILL);
    }
}

/// Insert the next chunk of rows. Returns true when the fill finished
/// (or was superseded by a newer one).
fn fill_tick(app: &mut App) -> bool {
    let total = match app.fill.as_ref() {
        Some(f) if f.gen == app.fill_gen => f.rows.len(),
        _ => {
            app.fill = None;
            return true;
        }
    };
    let tree = app.ctrl.tree;
    // Freeze painting for the whole chunk: without WM_SETREDRAW each of the
    // thousands of TVM_INSERTITEMs triggers a repaint/layout, turning a
    // ~100ms fill into seconds and making typing stutter during load.
    // This was the single biggest TreeView bottleneck for 5MB files.
    set_redraw(tree, false);
    let end = {
        let fill = app.fill.as_mut().unwrap();
        let end = (fill.next + FILL_CHUNK).min(fill.rows.len());
        // Pre-reserve map capacity once per fill to avoid repeated rehashing
        // while inserting tens of thousands of nodes.
        if fill.next == 0 {
            let cap = fill.rows.len();
            app.item_to_id.reserve(cap);
            app.id_to_item.reserve(cap);
            app.item_type.reserve(cap);
            app.container_items.reserve(cap / 4);
        }
        while fill.next < end {
            // Clone this row's short display strings, then release the borrow
            // before mutating the stack/maps.
            let row: FlatRow = fill.rows[fill.next].clone();
            while fill.stack.last().map(|(d, _)| *d >= row.depth).unwrap_or(false) {
                fill.stack.pop();
            }
            let parent = fill.stack.last().map(|(_, it)| *it).unwrap_or(TVI_ROOT.0 as isize);
            let item = tree_insert_raw(tree, parent, TVI_LAST.0 as isize, &row.label);
            if item != 0 {
                app.item_to_id.insert(item, row.id.clone());
                app.id_to_item.insert(row.id.clone(), item);
                app.item_type.insert(item, row.type_name);
                if row.is_container {
                    app.container_items.push(item);
                    fill.stack.push((row.depth, item));
                }
            }
            fill.next += 1;
        }
        fill.next
    };
    set_redraw(tree, true);
    if end >= total {
        app.fill = None;
        app.tree_complete = true;
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(app.hwnd, TIMER_FILL);
        }
        // Default: only root expanded (classic JSON Viewer look). Re-apply
        // every branch the user had open before a Split-mode rebuild. IDs that
        // no longer exist after the parse are dropped so they cannot leak
        // into future documents.
        if let Some(&root_item) = app.id_to_item.get("$") {
            tree_expand_by_hwnd(tree, root_item, true);
        }
        app.expanded_ids
            .retain(|id| id == "$" || app.id_to_item.contains_key(id));
        let restore: Vec<String> = app.expanded_ids.iter().cloned().collect();
        for id in restore {
            if id == "$" {
                continue;
            }
            if let Some(&item) = app.id_to_item.get(&id) {
                tree_expand_by_hwnd(tree, item, true);
            }
        }
        if app.expand_on_fill_done {
            app.expand_on_fill_done = false;
            expand_all(app);
        }
        if let Some(sel) = app.model.selected_id.clone() {
            reveal_id(app, &sel, false);
        }
        refresh_grid(app);
        // Split-mode re-parses rebuild the tree from scratch; restore the
        // inline long-text panel for the re-selected node and its layout.
        refresh_expand_panel(app);
        layout(app);
        refresh_status(app);
        true
    } else {
        unsafe {
            set_window_text(
                app.ctrl.status,
                &format!("Loading tree… {} of {} nodes", end, total),
            );
        }
        false
    }
}

/// Select `id` in the tree, expanding ancestors first (search results,
/// property-grid jumps, restored selection).
fn reveal_id(app: &mut App, id: &str, ensure_visible: bool) {
    let ancestors = app
        .model
        .find_with_ancestors(id)
        .map(|(a, _)| a)
        .unwrap_or_default();
    for a in &ancestors {
        if let Some(&ai) = app.id_to_item.get(a) {
            tree_expand_by_hwnd(app.ctrl.tree, ai, true);
        }
    }
    if let Some(&item) = app.id_to_item.get(id) {
        send(app.ctrl.tree, TVM_SELECTITEM, TVGN_CARET as usize, item);
        if ensure_visible {
            send(app.ctrl.tree, TVM_ENSUREVISIBLE, 0, item);
        }
    }
}

fn expand_all(app: &App) {
    set_redraw(app.ctrl.tree, false);
    for i in 0..app.container_items.len() {
        tree_expand(app, app.container_items[i], true);
    }
    set_redraw(app.ctrl.tree, true);
}

fn collapse_all(app: &App) {
    set_redraw(app.ctrl.tree, false);
    let root = app.id_to_item.get("$").copied();
    for i in 0..app.container_items.len() {
        let it = app.container_items[i];
        if Some(it) != root {
            tree_expand(app, it, false);
        }
    }
    if let Some(r) = root {
        tree_expand(app, r, true);
    }
    set_redraw(app.ctrl.tree, true);
}

fn refresh_grid(app: &mut App) {
    app.syncing_grid = true;
    set_redraw(app.ctrl.grid, false);
    send(app.ctrl.grid, LVM_DELETEALLITEMS, 0, 0);

    // Cap while the model builds the rows: a root array of 200k values should
    // not allocate 200k display strings just to show the first 10k.
    let (parent, total, all) = app.model.properties_for_selected_limited(GRID_CAP);
    app.grid_parent = parent;
    app.grid_total = total;

    let q = app.prop_filter.to_lowercase();
    let mut rows: Vec<PropertyRow> = if q.is_empty() {
        all
    } else {
        all.into_iter()
            .filter(|r| {
                r.name.to_lowercase().contains(&q)
                    || r.value.to_lowercase().contains(&q)
                    || r.typ.to_lowercase().contains(&q)
            })
            .collect()
    };
    // The ListView is not virtualized: cap pathological selections so the UI
    // can never freeze, and say so in the status bar.
    app.grid_truncated = if total > GRID_CAP { Some(total) } else { None };
    if rows.len() > GRID_CAP {
        rows.truncate(GRID_CAP);
    }
    app.grid_rows = rows.clone();

    // Header: selected node name, "← Parent" jump, filter, item count.
    let title = match app.model.selected_node() {
        Some(sel) if sel.path == "$" => "JSON (Root)".to_string(),
        Some(sel) if sel.is_container() => sel.key.clone(),
        Some(sel) => sel.key.clone(),
        None => "JSON".to_string(),
    };
    unsafe {
        set_window_text(app.ctrl.prop_title, &format!("Properties  |  {}", title));
        set_window_text(
            app.ctrl.prop_count,
            &if q.is_empty() {
                format!("{} items", total)
            } else {
                format!("{} of {} items", rows.len(), total)
            },
        );
        match &app.grid_parent {
            Some(parent) => {
                let label = if parent.path == "$" { "← Root" } else { "← Parent" };
                set_window_text(app.ctrl.prop_parent, label);
                let _ = EnableWindow(app.ctrl.prop_parent, BOOL(1));
            }
            None => {
                set_window_text(app.ctrl.prop_parent, "← Parent");
                let _ = EnableWindow(app.ctrl.prop_parent, BOOL(0));
            }
        }
        let _ = ShowWindow(app.ctrl.prop_parent, SW_SHOW);
        let _ = ShowWindow(app.ctrl.prop_title, SW_SHOW);
        let _ = ShowWindow(app.ctrl.prop_count, SW_SHOW);
        let _ = ShowWindow(app.ctrl.prop_filter_label, SW_SHOW);
        let _ = ShowWindow(app.ctrl.prop_filter, SW_SHOW);
    }

    for (i, row) in rows.iter().enumerate() {
        let name = wide(&row.name);
        let mut item = LVITEMW::default();
        item.mask = LVIF_TEXT;
        item.iItem = i as i32;
        item.iSubItem = 0;
        item.pszText = windows::core::PWSTR(name.as_ptr() as *mut u16);
        send(app.ctrl.grid, LVM_INSERTITEMW, 0, &item as *const _ as isize);
        let value = wide(&row.value);
        let mut sub = LVITEMW::default();
        sub.iItem = i as i32;
        sub.iSubItem = 1;
        sub.pszText = windows::core::PWSTR(value.as_ptr() as *mut u16);
        send(app.ctrl.grid, LVM_SETITEMTEXTW, i, &sub as *const _ as isize);
        let typ = wide(&row.typ);
        let mut sub2 = LVITEMW::default();
        sub2.iItem = i as i32;
        sub2.iSubItem = 2;
        sub2.pszText = windows::core::PWSTR(typ.as_ptr() as *mut u16);
        send(app.ctrl.grid, LVM_SETITEMTEXTW, i, &sub2 as *const _ as isize);
    }
    set_redraw(app.ctrl.grid, true);
    app.syncing_grid = false;
}

/// Size the 3 grid columns to exactly fill the grid client width, so no
/// phantom blank 4th column/empty header space shows on the right.
/// Split: Property 36% / Value 44% / Type rest.
fn resize_grid_columns(app: &App) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
        let mut rc = RECT::default();
        if GetClientRect(app.ctrl.grid, &mut rc).is_err() {
            return;
        }
        let w = rc.right - rc.left;
        if w <= 60 {
            return;
        }
        let prop = (w * 36 / 100).max(120);
        let val = (w * 44 / 100).max(120);
        let typ = (w - prop - val).max(70);
        send(app.ctrl.grid, LVM_SETCOLUMNWIDTH, 0, prop as isize);
        send(app.ctrl.grid, LVM_SETCOLUMNWIDTH, 1, val as isize);
        send(app.ctrl.grid, LVM_SETCOLUMNWIDTH, 2, typ as isize);
    }
}

// ---------------- inline expand panel (macOS parity) ----------------
// macOS shows long strings inline as a full-width wrapped textarea below the
// row (with char count + Copy + collapse). Windows previously opened a
// separate popup window — this panel docks the same content inside the main
// window, under the tree, so there is no extra OS window.

/// Currently selected long-text leaf, if any: (key, full_value).
fn selected_long_text(app: &App) -> Option<(String, String)> {
    let id = app.model.selected_id.as_ref()?;
    let node = app.model.find_node(id)?;
    match &node.value {
        crate::json::JSONValue::Str(s) if node.is_long_text() => {
            Some((node.key.clone(), s.clone()))
        }
        _ => None,
    }
}

fn expand_header_text(key: &str, full: &str) -> (String, usize) {
    let chars = full.encode_utf16().count();
    let short: String = if key.chars().count() > 40 {
        format!("{}…", key.chars().take(40).collect::<String>())
    } else {
        key.to_string()
    };
    (format!("{} • {} chars", short, chars), chars)
}

/// True when the inline panel should be visible and laid out.
fn expand_should_show(app: &App) -> bool {
    if matches!(app.model.active_tab, AppTab::Text) {
        return false;
    }
    if !app.expand_visible {
        return false;
    }
    match (&app.expand_id, &app.model.selected_id) {
        (Some(ep), Some(sel)) if ep == sel => selected_long_text(app).is_some(),
        _ => false,
    }
}

/// Fill the panel widgets from the currently selected long-text node.
/// Call after selection changes; caller should then call `layout(app)`.
fn refresh_expand_panel(app: &mut App) {
    if matches!(app.model.active_tab, AppTab::Text) {
        return;
    }
    let sel = app.model.selected_id.clone();
    let long = selected_long_text(app);
    match (sel, long) {
        (Some(p), Some((k, full))) => {
            if app.expand_id.as_ref() != Some(&p) {
                // New long node selected → auto-expand inline (mac parity:
                // the full-width text appears without opening anything).
                app.expand_id = Some(p);
                app.expand_visible = true;
            }
            let (header, _) = expand_header_text(&k, &full);
            unsafe {
                set_window_text(app.ctrl.expand_label, &header);
                set_editor_text(app.ctrl.expand_text, &full);
            }
        }
        _ => {
            // Non-long selection: keep flags (so going back restores), but
            // `expand_should_show` will be false so layout hides the panel.
        }
    }
}

fn show_expand_inline(app: &mut App, id: String) {
    app.expand_id = Some(id.clone());
    app.expand_visible = true;
    app.model.selected_id = Some(id.clone());
    if let Some(n) = app.model.find_node(&id) {
        if let crate::json::JSONValue::Str(s) = &n.value {
            let (header, _) = expand_header_text(&n.key, s);
            let full = s.clone();
            unsafe {
                set_window_text(app.ctrl.expand_label, &header);
                set_editor_text(app.ctrl.expand_text, &full);
            }
        }
    }
    layout(app);
}

fn hide_expand_inline(app: &mut App) {
    app.expand_visible = false;
    layout(app);
}

fn toggle_expand_inline(app: &mut App, id: String) {
    if app.expand_visible && app.expand_id.as_ref() == Some(&id) {
        hide_expand_inline(app);
    } else {
        show_expand_inline(app, id);
    }
}

fn copy_expand_text(app: &App) {
    let full: Option<String> = app
        .expand_id
        .as_ref()
        .and_then(|p| app.model.find_node(p))
        .and_then(|n| match &n.value {
            crate::json::JSONValue::Str(s) => Some(s.clone()),
            _ => None,
        });
    if let Some(text) = full {
        if clipboard::set_text(&text) {
            unsafe {
                set_window_text(app.ctrl.status, "Copied!");
            }
        }
    }
}

/// Mirror `model.search_results` into the draw-fast `search_marks` set and
/// repaint the tree so old yellow rows clear and new ones appear.
fn sync_search_marks(app: &mut App) {
    app.search_marks.clear();
    app.search_marks
        .extend(app.model.search_results.iter().cloned());
    unsafe {
        let _ = InvalidateRect(app.ctrl.tree, None, BOOL(1));
    }
}

/// Scroll the TreeView so the model-selected node is visible. Called AFTER
/// `layout(app)` — resizing the tree first can reset the scroll position,
/// which is why search jumps previously landed on the right node (grid /
/// expand panel / status all correct) yet the tree still showed the top.
fn ensure_selected_visible(app: &App) {
    let item = app
        .model
        .selected_id
        .as_ref()
        .and_then(|id| app.id_to_item.get(id).copied());
    if let Some(it) = item {
        send(app.ctrl.tree, TVM_ENSUREVISIBLE, 0, it);
    }
}

/// Put the tree viewport back at the root (used when clearing search, mirroring
/// the macOS/Linux behavior).
fn scroll_tree_to_root(app: &App) {
    if let Some(&root) = app.id_to_item.get("$") {
        tree_expand_by_hwnd(app.ctrl.tree, root, true);
        send(
            app.ctrl.tree,
            TVM_SELECTITEM,
            TVGN_FIRSTVISIBLE as usize,
            root,
        );
    }
}

fn clear_search_marks_and_repaint(app: &mut App) {
    app.search_marks.clear();
    unsafe {
        let _ = InvalidateRect(app.ctrl.tree, None, BOOL(1));
    }
}

/// Clear search state and return both the box and the tree viewport to root.
fn clear_search_ui(app: &mut App) {
    app.model.clear_search();
    clear_search_marks_and_repaint(app);
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(app.hwnd, TIMER_SEARCH);
        set_window_text(app.ctrl.search_edit, "");
    }
    scroll_tree_to_root(app);
    refresh_status(app);
}

fn refresh_status(app: &App) {
    unsafe {
        let mut text = format!("{}    |    {}", app.model.status_text(), app.model.metrics_text());
        if app.tree_stale && app.model.root.is_some() {
            text.push_str("    |    Tree out of date — press Refresh to update");
        }
        if !app.prop_filter.is_empty() {
            text.push_str(&format!(
                "    |    properties: {} of {} match the filter",
                app.grid_rows.len(),
                app.grid_total
            ));
        }
        if let Some(total) = app.grid_truncated {
            text.push_str(&format!("    |    grid shows first {} of {} properties", GRID_CAP, total));
        }
        set_window_text(app.ctrl.status, &text);
        let path = app.model.selected_path().unwrap_or_default();
        set_window_text(app.ctrl.path, &path);
        set_window_text(app.ctrl.search_status, &app.model.search_status);
    }
}

fn refresh_editor(app: &mut App) {
    unsafe {
        app.syncing_editor = true;
        set_editor_text(app.ctrl.editor, &app.model.raw_text);
        app.syncing_editor = false;
    }
}

fn show_error(hwnd: HWND, msg: &str) {
    unsafe {
        let t = wide(msg);
        let c = wide("JSON error");
        MessageBoxW(hwnd, windows::core::PCWSTR::from_raw(t.as_ptr()), windows::core::PCWSTR::from_raw(c.as_ptr()), MB_ICONERROR | MB_OK);
    }
}

fn show_info(hwnd: HWND, title: &str, msg: &str) {
    unsafe {
        let t = wide(msg);
        let c = wide(title);
        MessageBoxW(hwnd, windows::core::PCWSTR::from_raw(t.as_ptr()), windows::core::PCWSTR::from_raw(c.as_ptr()), MB_ICONINFORMATION | MB_OK);
    }
}

// Pull the editor control's current text into the model (metrics included).
// Called on a short idle timer and before every action — never per keystroke,
// so typing stays O(1) no matter the file size.
fn pull_editor_text(app: &mut App) -> bool {
    unsafe {
        if app.syncing_editor {
            return false;
        }
        let text = get_edit_text(app.ctrl.editor);
        if text != app.model.raw_text {
            app.model.raw_text = text;
            app.model.mark_edited();
            // Split shows the previous tree while the editor holds text we
            // have not parsed yet (or cannot parse). Flag it as stale until a
            // successful parse lands.
            if matches!(app.model.active_tab, AppTab::Split) && app.model.root.is_some() {
                app.tree_stale = true;
            }
            true
        } else {
            false
        }
    }
}

// Re-parse editor text and refill the tree (chunked — returns immediately).
// Returns true when the text parsed.
fn parse_and_view(app: &mut App, silent: bool) -> bool {
    let settings = app.settings.lock().unwrap().clone();
    pull_editor_text(app);
    let ok = app.model.parse_and_build_tree(silent, &settings);
    if ok {
        app.tree_stale = false;
        begin_tree_fill(app, true);
    } else {
        // Keep the last successfully parsed tree visible while the editor is
        // invalid (macOS split-mode parity), but make the stale state obvious.
        app.tree_stale = app.model.root.is_some();
        if !silent && !app.model.error_message.is_empty() {
            let msg = app.model.error_message.clone();
            show_error(app.hwnd, &msg);
        }
        refresh_status(app);
    }
    ok
}

/// Refill the tree when the model already parsed (transforms/paste/open call
/// `parse_and_build_tree` internally). Calling `parse_and_view` here would
/// parse the same 5MB text a second time (~300ms wasted) and freeze typing.
/// `preserve_nav` keeps the selected node and expanded branches (Split mode)
/// instead of collapsing everything for a new document.
fn refill_tree_after_model_change(app: &mut App, preserve_nav: bool) {
    if app.model.root.is_some() {
        app.tree_stale = false;
        begin_tree_fill(app, preserve_nav);
    } else {
        cancel_tree_fill(app);
        app.tree_stale = false;
        app.tree_complete = true;
        refresh_status(app);
    }
}

// ---------------- actions ----------------

fn do_copy(app: &mut App, kind: &str) {
    pull_editor_text(app);
    let settings = app.settings.lock().unwrap().clone();
    let text = match kind {
        "beautified" => app.model.copy_beautified(&settings),
        "minified" => app.model.copy_minified(),
        "stringified" => app.model.copy_stringified(&settings),
        "python" => app.model.copy_python(&settings),
        _ => app.model.copy_text(),
    };
    if clipboard::set_text(&text) {
        unsafe {
            let label = match kind {
                "beautified" => "Copied Beautified JSON!",
                "minified" => "Copied Minified JSON!",
                "stringified" => "Copied Stringified JSON!",
                "python" => "Copied Python Dictionary!",
                _ => "Copied Text!",
            };
            set_window_text(app.ctrl.status, label);
        }
    }
}

fn do_paste(app: &mut App) {
    if let Some(text) = clipboard::get_text() {
        let settings = app.settings.lock().unwrap().clone();
        let res = app.model.paste(&text, &settings);
        refresh_editor(app);
        refill_tree_after_model_change(app, false);
        if res == "converted-python" {
            unsafe { set_window_text(app.ctrl.status, "Converted Python Dictionary to JSON!"); }
        }
    }
}

fn do_transform(app: &mut App, kind: &str) {
    // Sync editor -> model first.
    unsafe {
        let text = get_edit_text(app.ctrl.editor);
        app.model.raw_text = text;
        app.model.mark_edited();
    }
    let settings = app.settings.lock().unwrap().clone();
    let result = match kind {
        "format" => app.model.beautify(&settings).map(|_| "Formatted JSON"),
        "minify" => app.model.minify(&settings).map(|_| "Minified JSON"),
        "j2p" => app.model.json_to_python(&settings).map(|_| "Converted JSON to Python!"),
        "p2j" => app.model.python_to_json(&settings).map(|_| "Converted Python to JSON!"),
        _ => Ok(""),
    };
    match result {
        Ok(label) => {
            refresh_editor(app);
            // format/minify/p2j already parsed inside the model — refilling
            // avoids parsing the same large text twice. j2p leaves Python in
            // the editor (no tree parse), so keep the old path there.
            if kind == "j2p" {
                parse_and_view(app, true);
            } else {
                refill_tree_after_model_change(app, true);
            }
            if !label.is_empty() {
                unsafe { set_window_text(app.ctrl.status, label); }
            }
        }
        Err(e) => show_error(app.hwnd, &e),
    }
}

fn do_stringify(app: &mut App) {
    unsafe {
        let text = get_edit_text(app.ctrl.editor);
        app.model.raw_text = text;
        app.model.mark_edited();
    }
    let settings = app.settings.lock().unwrap().clone();
    app.model.stringify(&settings);
    refresh_editor(app);
    refill_tree_after_model_change(app, true);
    unsafe { set_window_text(app.ctrl.status, "Stringified JSON"); }
}

fn do_unescape(app: &mut App) {
    unsafe {
        let text = get_edit_text(app.ctrl.editor);
        app.model.raw_text = text;
        app.model.mark_edited();
    }
    let settings = app.settings.lock().unwrap().clone();
    app.model.unescape(&settings);
    refresh_editor(app);
    refill_tree_after_model_change(app, true);
    unsafe { set_window_text(app.ctrl.status, "Unescaped JSON"); }
}

fn do_open(app: &mut App) {
    unsafe {
        let mut file = [0u16; 1024];
        let filter = wide("JSON Files (*.json)\0*.json\0All Files (*.*)\0*.*\0\0");
        let title = wide("Open JSON File");
        let mut ofn = OPENFILENAMEW::default();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = app.hwnd;
        ofn.lpstrFile = windows::core::PWSTR(file.as_mut_ptr());
        ofn.nMaxFile = file.len() as u32;
        ofn.lpstrFilter = windows::core::PCWSTR::from_raw(filter.as_ptr());
        ofn.nFilterIndex = 1;
        ofn.lpstrTitle = windows::core::PCWSTR::from_raw(title.as_ptr());
        ofn.Flags = OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;
        if GetOpenFileNameW(&mut ofn).as_bool() {
            let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
            let path = String::from_utf16_lossy(&file[..len]);
            let settings = app.settings.lock().unwrap().clone();
            match app.model.load_file(std::path::Path::new(&path), &settings) {
                Ok(_) => {
                    if let Some(name) = std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                    {
                        set_window_text(app.hwnd, &format!("JSON Viewer — {}", name));
                    }
                    refresh_editor(app);
                    app.expanded_ids.clear();
                    refill_tree_after_model_change(app, false);
                }
                Err(e) => show_error(app.hwnd, &e),
            }
        }
    }
}

fn do_save(app: &mut App) {
    unsafe {
        // Sync editor first so saves include latest typing.
        let text = get_edit_text(app.ctrl.editor);
        app.model.raw_text = text;
        app.model.mark_edited();
        refresh_status(app);
        let mut file_buf = wide("document.json");
        file_buf.resize(1024, 0);
        let filter = wide("JSON Files (*.json)\0*.json\0All Files (*.*)\0*.*\0\0");
        let title = wide("Save JSON File");
        let ext = wide("json");
        let mut ofn = OPENFILENAMEW::default();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = app.hwnd;
        ofn.lpstrFile = windows::core::PWSTR(file_buf.as_mut_ptr());
        ofn.nMaxFile = file_buf.len() as u32;
        ofn.lpstrFilter = windows::core::PCWSTR::from_raw(filter.as_ptr());
        ofn.nFilterIndex = 1;
        ofn.lpstrTitle = windows::core::PCWSTR::from_raw(title.as_ptr());
        ofn.lpstrDefExt = windows::core::PCWSTR::from_raw(ext.as_ptr());
        ofn.Flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT;
        if GetSaveFileNameW(&mut ofn).as_bool() {
            let len = file_buf.iter().position(|&c| c == 0).unwrap_or(file_buf.len());
            let path = String::from_utf16_lossy(&file_buf[..len]);
            match app.model.save_file(std::path::Path::new(&path)) {
                Ok(_) => set_window_text(app.ctrl.status, &format!("Saved to {}", path)),
                Err(e) => show_error(app.hwnd, &e),
            }
        }
    }
}

/// Reveal the model-selected node after a search / navigation jump.
fn reveal_selected(app: &mut App, ensure_visible: bool) {
    if let Some(id) = app.model.selected_id.clone() {
        reveal_id(app, &id, ensure_visible);
        refresh_grid(app);
        refresh_expand_panel(app);
        layout(app);
        // Resize first, scroll second: layout can reset the TreeView scroll
        // position, so ensuring before layout left the right node selected
        // (grid/expand/status correct) but the tree still showing the top.
        if ensure_visible {
            ensure_selected_visible(app);
        }
    }
}

fn start_search_poll(app: &App) {
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(app.hwnd, TIMER_SEARCH, 40, None);
    }
}

fn stop_search_poll(app: &App) {
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(app.hwnd, TIMER_SEARCH);
    }
}

fn do_search(app: &mut App, dir: i32) {
    pull_editor_text(app);
    unsafe {
        let q = get_window_text(app.ctrl.search_edit);
        app.model.search_query = q;
    }
    let settings = app.settings.lock().unwrap().clone();
    if app.model.search_query.trim().is_empty() {
        clear_search_ui(app);
        return;
    }
    if dir == 0 {
        // GO: fresh search on the cancellable background worker.
        app.model.search_start(&settings, false);
    } else if dir > 0 {
        app.model.search_next(&settings);
    } else {
        app.model.search_previous(&settings);
    }
    sync_search_marks(app);
    if app.model.search_in_flight {
        start_search_poll(app);
        refresh_status(app);
    } else {
        stop_search_poll(app);
        reveal_selected(app, true);
        refresh_status(app);
    }
}

/// Apply a completed background search (called from TIMER_SEARCH).
fn apply_search_completion(app: &mut App) {
    if !app.model.poll_search_results() {
        if !app.model.search_in_flight {
            stop_search_poll(app);
            refresh_status(app);
        }
        return;
    }
    sync_search_marks(app);
    reveal_selected(app, true);
    refresh_status(app);
    if !app.model.search_in_flight {
        stop_search_poll(app);
    }
}

fn set_tab(app: &mut App, tab: AppTab) {
    if matches!(tab, AppTab::Viewer | AppTab::Split) {
        // Avoid re-parsing (and a 5MB editor copy) when nothing changed:
        // `parse_and_view` pulls the full editor text + re-parses + reflattens
        // (~300ms for 5MB). Only do it when dirty or missing a tree.
        // `tree_complete` guards the cancelled-partial-fill case: hiding the
        // tree mid-fill leaves partial items, so returning must restart.
        if app.model.is_dirty || app.model.root.is_none() {
            parse_and_view(app, true);
        } else if app.fill.is_some() {
            // A fill is already streaming — let it continue.
        } else if !app.tree_complete {
            begin_tree_fill(app, true);
        }
    } else {
        // Tree hidden: stop any in-flight fill to save work.
        cancel_tree_fill(app);
    }
    app.model.active_tab = tab;
    refresh_expand_panel(app);
    layout(app);
    if matches!(tab, AppTab::Viewer | AppTab::Split) {
        refresh_status(app);
        if app.tree_complete {
            ensure_selected_visible(app);
        }
    }
    // Repaint the tab buttons: owner-drawn highlight reads `active_tab` at
    // WM_DRAWITEM time, and moving/resizing alone doesn't always invalidate
    // them — without this the old tab keeps its accent (several tabs look
    // "selected" at once).
    unsafe {
        for item in app.ctrl.row_tabs.iter() {
            if !item.hwnd.is_invalid() {
                let _ = InvalidateRect(item.hwnd, None, BOOL(1));
            }
        }
    }
}

fn do_zoom(app: &mut App, delta: i32) {
    {
        let mut s = app.settings.lock().unwrap();
        if delta == 0 {
            s.font_size = 12.0;
        } else if delta > 0 {
            s.font_size = (s.font_size + 1.0).min(24.0);
        } else {
            s.font_size = (s.font_size - 1.0).max(9.0);
        }
    }
    save_settings(app);
    rebuild_font(app);
}

fn save_settings(app: &App) {
    let s = app.settings.lock().unwrap().clone().clamped();
    *app.settings.lock().unwrap() = s.clone();
    app.store.lock().unwrap().settings = s;
    app.store.lock().unwrap().save();
}

fn rebuild_font(app: &mut App) {
    let size = app.settings.lock().unwrap().font_size;
    unsafe {
        let old = app.font_mono;
        app.font_mono = make_font("Consolas", size);
        apply_font_to_controls(app);
        if !old.is_invalid() {
            let _ = DeleteObject(old);
        }
    }
}

fn apply_wrap_style(app: &App) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_STYLE, SetWindowPos, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER};
        let wrap = app.settings.lock().unwrap().wrap_lines;
        let mut style = GetWindowLongPtrW(app.ctrl.editor, GWL_STYLE) as u32;
        if wrap {
            style &= !(WS_HSCROLL.0 | ES_AUTOHSCROLL as u32);
        } else {
            style |= WS_HSCROLL.0 | ES_AUTOHSCROLL as u32;
        }
        SetWindowLongPtrW(app.ctrl.editor, GWL_STYLE, style as isize);
        let _ = SetWindowPos(app.ctrl.editor, HWND(std::ptr::null_mut()), 0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
    }
}

fn show_shortcuts(hwnd: HWND) {
    show_info(hwnd, "Keyboard Shortcuts",
        "General:\r\n  Ctrl+,  Settings\r\n  Ctrl+1/2/3  Viewer / Text / Split tabs\r\n\r\nTree Viewer:\r\n  Ctrl+E  Expand all    Ctrl+Shift+E  Collapse all\r\n  Ctrl+Alt+P  Toggle properties panel\r\n  Click node to inspect - double-click grid row to jump\r\n  Right-click node for copy / expand menu\r\n  Double-click a long value to expand it inline below the tree\r\n\r\nEditor:\r\n  Ctrl+X / Ctrl+C / Ctrl+V / Ctrl+A  Cut / Copy / Paste / Select all\r\n  (Paste auto-converts Python dicts to JSON)\r\n  Ctrl+K  Clear    Ctrl+O  Open    Ctrl+S  Save\r\n  Ctrl+Alt+C  Copy as Python dictionary\r\n\r\nSearch:\r\n  Ctrl+F  Focus search    Enter  Next    Shift+Enter  Previous\r\n  Ctrl+G / Ctrl+Shift+G  Next / Previous match\r\n  Esc  Clear search");
}

fn show_about(hwnd: HWND) {
    show_info(hwnd, "About JSON Viewer",
        "JSON Viewer 1.0.0 for Windows\r\n\r\nA fast, lightweight, native JSON viewer and formatter.\r\nWindows port of the macOS JSON Viewer app.\r\n\r\nFeatures: tree explorer, property inspector, formatting,\r\nminification, stringify/unescape, Python dict conversion,\r\nfull-text search with jump navigation.");
}

// ---------------- layout ----------------

/// Two compact rows above the property grid: title/count, then parent jump +
/// filter. Returns the Y where the ListView itself should start.
const PROP_HEADER_H: i32 = 56;

fn layout_prop_header(app: &App, x: i32, y: i32, w: i32) -> i32 {
    unsafe {
        for h in [
            app.ctrl.prop_title,
            app.ctrl.prop_parent,
            app.ctrl.prop_filter_label,
            app.ctrl.prop_filter,
            app.ctrl.prop_count,
        ] {
            let _ = ShowWindow(h, SW_SHOW);
        }
        let count_w = 78;
        let title_w = (w - count_w - 6).max(60);
        let _ = SetWindowPos(
            app.ctrl.prop_title,
            HWND(std::ptr::null_mut()),
            x,
            y,
            title_w,
            22,
            SWP_NOZORDER,
        );
        let _ = SetWindowPos(
            app.ctrl.prop_count,
            HWND(std::ptr::null_mut()),
            x + w - count_w,
            y,
            count_w,
            22,
            SWP_NOZORDER,
        );

        let parent_w = 84;
        let label_w = 38;
        let filter_w = (w - parent_w - label_w - 10).max(60);
        let row_y = y + 26;
        let _ = SetWindowPos(
            app.ctrl.prop_parent,
            HWND(std::ptr::null_mut()),
            x,
            row_y,
            parent_w,
            24,
            SWP_NOZORDER,
        );
        let _ = SetWindowPos(
            app.ctrl.prop_filter_label,
            HWND(std::ptr::null_mut()),
            x + parent_w + 4,
            row_y,
            label_w,
            24,
            SWP_NOZORDER,
        );
        let _ = SetWindowPos(
            app.ctrl.prop_filter,
            HWND(std::ptr::null_mut()),
            x + parent_w + 4 + label_w,
            row_y,
            filter_w,
            24,
            SWP_NOZORDER,
        );
    }
    y + PROP_HEADER_H
}

fn hide_prop_header(app: &App) {
    unsafe {
        for h in [
            app.ctrl.prop_title,
            app.ctrl.prop_parent,
            app.ctrl.prop_filter_label,
            app.ctrl.prop_filter,
            app.ctrl.prop_count,
        ] {
            let _ = ShowWindow(h, SW_HIDE);
        }
    }
}

fn layout(app: &App) {
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
        let mut rc = RECT::default();
        let _ = GetClientRect(app.hwnd, &mut rc);
        let w = rc.right - rc.left;
        let _h = rc.bottom - rc.top;

        const ROW_H: i32 = 26;
        const ROW1_Y: i32 = 4;
        const ROW2_Y: i32 = 34;
        let search_h = 30;
        let status_h = 24;
        let path_h = 22;

        // Row 1 (macOS toolbar): tab switcher on the left, utilities right-aligned.
        layout_bar(&app.ctrl.row_tabs, 8, ROW1_Y, ROW_H, true);
        let utils_w: i32 = app.ctrl.row_utils.iter().map(|b| b.width + 4).sum();
        layout_bar(&app.ctrl.row_utils, (w - 8 - utils_w).max(300), ROW1_Y, ROW_H, true);

        // Row 2 (macOS Text/Tree toolbars): contextual actions for the active tab.
        let show_text_row = !matches!(app.model.active_tab, AppTab::Viewer);
        layout_bar(&app.ctrl.row_text, 8, ROW2_Y, ROW_H, show_text_row);
        layout_bar(&app.ctrl.row_viewer, 8, ROW2_Y, ROW_H, !show_text_row);

        let content_y = ROW2_Y + ROW_H + 6;
        let search_visible = !matches!(app.model.active_tab, AppTab::Text);
        let bottom_reserved = status_h + path_h + if search_visible { search_h } else { 0 };
        let content_h = (_h - content_y - bottom_reserved).max(60);

        let show = |hwnd: HWND, visible: bool| {
            let _ = ShowWindow(hwnd, if visible { windows::Win32::UI::WindowsAndMessaging::SW_SHOW } else { windows::Win32::UI::WindowsAndMessaging::SW_HIDE });
        };

        match app.model.active_tab {
            AppTab::Text => {
                show(app.ctrl.editor, true);
                show(app.ctrl.tree, false);
                show(app.ctrl.grid, false);
                show(app.ctrl.expand_label, false);
                show(app.ctrl.expand_text, false);
                show(app.ctrl.expand_copy, false);
                show(app.ctrl.expand_collapse, false);
                hide_prop_header(app);
                let _ = SetWindowPos(app.ctrl.editor, HWND(std::ptr::null_mut()), 8, content_y, w - 16, content_h, SWP_NOZORDER);
            }
            AppTab::Viewer => {
                show(app.ctrl.editor, false);
                show(app.ctrl.tree, true);
                let grid_on = app.props_visible;
                show(app.ctrl.grid, grid_on);
                let expand_on = expand_should_show(app);
                // Inline panel height: ~35% of content, clamped — like the mac
                // full-width box (large enough to read, small enough to keep
                // tree context). Guarded so tiny windows can't get negative sizes.
                let expand_h = if expand_on {
                    (((content_h * 35) / 100).clamp(120, 260)).min((content_h - 80).max(80))
                } else {
                    0
                };
                let tree_h = if expand_on { (content_h - expand_h - 8).max(60) } else { content_h };
                // Helper lays out tree + expand panel inside [x, x+tree_w).
                let place_tree_with_expand = |tree_x: i32, tree_w: i32| {
                    let _ = SetWindowPos(app.ctrl.tree, HWND(std::ptr::null_mut()), tree_x, content_y, tree_w, tree_h, SWP_NOZORDER);
                    show(app.ctrl.expand_label, expand_on);
                    show(app.ctrl.expand_text, expand_on);
                    show(app.ctrl.expand_copy, expand_on);
                    show(app.ctrl.expand_collapse, expand_on);
                    if expand_on {
                        let ey = content_y + tree_h + 8;
                        // Header: label left, Copy + Collapse right-aligned.
                        let copy_w = 70;
                        let collapse_w = 80;
                        let label_w = (tree_w - copy_w - collapse_w - 16).max(80);
                        let _ = SetWindowPos(app.ctrl.expand_label, HWND(std::ptr::null_mut()), tree_x, ey, label_w, 24, SWP_NOZORDER);
                        let _ = SetWindowPos(app.ctrl.expand_copy, HWND(std::ptr::null_mut()), tree_x + label_w + 6, ey, copy_w, 24, SWP_NOZORDER);
                        let _ = SetWindowPos(app.ctrl.expand_collapse, HWND(std::ptr::null_mut()), tree_x + label_w + 6 + copy_w + 4, ey, collapse_w, 24, SWP_NOZORDER);
                        let _ = SetWindowPos(app.ctrl.expand_text, HWND(std::ptr::null_mut()), tree_x, ey + 26, tree_w, (expand_h - 28).max(40), SWP_NOZORDER);
                    }
                };
                if grid_on {
                    let tree_w = ((w - 24) * 68) / 100;
                    place_tree_with_expand(8, tree_w);
                    let grid_x = 16 + tree_w;
                    let grid_w = w - 24 - tree_w;
                    let grid_y = layout_prop_header(app, grid_x, content_y, grid_w);
                    let _ = SetWindowPos(app.ctrl.grid, HWND(std::ptr::null_mut()), grid_x, grid_y, grid_w, content_h - PROP_HEADER_H, SWP_NOZORDER);
                } else {
                    hide_prop_header(app);
                    place_tree_with_expand(8, w - 16);
                }
            }
            AppTab::Split => {
                show(app.ctrl.editor, true);
                show(app.ctrl.tree, true);
                let grid_on = app.props_visible;
                show(app.ctrl.grid, grid_on);
                let expand_on = expand_should_show(app);
                let expand_h = if expand_on {
                    (((content_h * 35) / 100).clamp(120, 260)).min((content_h - 80).max(80))
                } else {
                    0
                };
                let tree_h = if expand_on { (content_h - expand_h - 8).max(60) } else { content_h };
                let edit_w = ((w - 24) * 40) / 100;
                let _ = SetWindowPos(app.ctrl.editor, HWND(std::ptr::null_mut()), 8, content_y, edit_w, content_h, SWP_NOZORDER);
                let rest_x = 16 + edit_w;
                let rest_w = w - 8 - rest_x;
                let place_tree_with_expand = |tree_x: i32, tree_w: i32| {
                    let _ = SetWindowPos(app.ctrl.tree, HWND(std::ptr::null_mut()), tree_x, content_y, tree_w, tree_h, SWP_NOZORDER);
                    show(app.ctrl.expand_label, expand_on);
                    show(app.ctrl.expand_text, expand_on);
                    show(app.ctrl.expand_copy, expand_on);
                    show(app.ctrl.expand_collapse, expand_on);
                    if expand_on {
                        let ey = content_y + tree_h + 8;
                        let copy_w = 70;
                        let collapse_w = 80;
                        let label_w = (tree_w - copy_w - collapse_w - 16).max(80);
                        let _ = SetWindowPos(app.ctrl.expand_label, HWND(std::ptr::null_mut()), tree_x, ey, label_w, 24, SWP_NOZORDER);
                        let _ = SetWindowPos(app.ctrl.expand_copy, HWND(std::ptr::null_mut()), tree_x + label_w + 6, ey, copy_w, 24, SWP_NOZORDER);
                        let _ = SetWindowPos(app.ctrl.expand_collapse, HWND(std::ptr::null_mut()), tree_x + label_w + 6 + copy_w + 4, ey, collapse_w, 24, SWP_NOZORDER);
                        let _ = SetWindowPos(app.ctrl.expand_text, HWND(std::ptr::null_mut()), tree_x, ey + 26, tree_w, (expand_h - 28).max(40), SWP_NOZORDER);
                    }
                };
                if grid_on {
                    let tree_w = (rest_w * 65) / 100;
                    place_tree_with_expand(rest_x, tree_w);
                    let grid_x = rest_x + tree_w + 8;
                    let grid_w = rest_w - tree_w - 8;
                    let grid_y = layout_prop_header(app, grid_x, content_y, grid_w);
                    let _ = SetWindowPos(app.ctrl.grid, HWND(std::ptr::null_mut()), grid_x, grid_y, grid_w, content_h - PROP_HEADER_H, SWP_NOZORDER);
                } else {
                    hide_prop_header(app);
                    place_tree_with_expand(rest_x, rest_w);
                }
            }
        }

        // Columns always exactly fill the grid (no phantom 4th column).
        if app.props_visible && !matches!(app.model.active_tab, AppTab::Text) {
            resize_grid_columns(app);
        }

        // Search bar (all controls share y/height; labels are vertically
        // centered via SS_CENTERIMAGE so baselines line up).
        let sy = content_y + content_h + 2;
        show(app.ctrl.search_label, search_visible);
        show(app.ctrl.search_edit, search_visible);
        show(app.ctrl.search_go, search_visible);
        show(app.ctrl.search_prev, search_visible);
        show(app.ctrl.search_next, search_visible);
        show(app.ctrl.search_status, search_visible);
        if search_visible {
            let _ = SetWindowPos(app.ctrl.search_label, HWND(std::ptr::null_mut()), 8, sy, 60, 24, SWP_NOZORDER);
            let _ = SetWindowPos(app.ctrl.search_edit, HWND(std::ptr::null_mut()), 70, sy, 260, 24, SWP_NOZORDER);
            let _ = SetWindowPos(app.ctrl.search_go, HWND(std::ptr::null_mut()), 336, sy, 60, 24, SWP_NOZORDER);
            let _ = SetWindowPos(app.ctrl.search_prev, HWND(std::ptr::null_mut()), 402, sy, 80, 24, SWP_NOZORDER);
            let _ = SetWindowPos(app.ctrl.search_next, HWND(std::ptr::null_mut()), 488, sy, 70, 24, SWP_NOZORDER);
            let _ = SetWindowPos(app.ctrl.search_status, HWND(std::ptr::null_mut()), 566, sy, w - 574, 24, SWP_NOZORDER);
        }
        let stat_y = sy + if search_visible { search_h } else { 4 };
        let _ = SetWindowPos(app.ctrl.status, HWND(std::ptr::null_mut()), 8, stat_y, w - 16, 22, SWP_NOZORDER);
        let _ = SetWindowPos(app.ctrl.path, HWND(std::ptr::null_mut()), 8, stat_y + 22, w - 16, 20, SWP_NOZORDER);
    }
}

// ---------------- menus & accelerators ----------------

unsafe fn build_menu() -> HMENU {
    let bar = CreateMenu().unwrap_or(HMENU(std::ptr::null_mut()));
    let popup = |items: &[(u32, &str)]| -> HMENU {
        let m = CreatePopupMenu().unwrap_or(HMENU(std::ptr::null_mut()));
        for (id, label) in items {
            if *id == 0 {
                let _ = AppendMenuW(m, MF_SEPARATOR, 0, None);
            } else {
                let w = wide(label);
                let _ = AppendMenuW(m, MF_STRING, *id as usize, windows::core::PCWSTR::from_raw(w.as_ptr()));
            }
        }
        m
    };
    let add_popup = |bar: HMENU, menu: HMENU, label: &str| {
        let w = wide(label);
        let _ = AppendMenuW(bar, MF_STRING | MF_POPUP, menu.0 as usize, windows::core::PCWSTR::from_raw(w.as_ptr()));
    };
    add_popup(bar, popup(&[
        (IDC_BTN_OPEN, "Open JSON File...\tCtrl+O"),
        (IDC_BTN_SAVE, "Save JSON File...\tCtrl+S"),
        (0, ""),
        (IDM_EXIT, "Exit"),
    ]), "&File");
    add_popup(bar, popup(&[
        (IDM_ACCEL_CUT, "Cu&t\tCtrl+X"),
        (IDM_ACCEL_COPY, "&Copy\tCtrl+C"),
        (IDM_ACCEL_PASTE, "&Paste\tCtrl+V"),
        (IDM_ACCEL_SELECTALL, "Select &All\tCtrl+A"),
        (0, ""),
        (IDC_BTN_FORMAT, "&Format (Beautify)"),
        (IDC_BTN_MINIFY, "&Minify"),
        (IDC_BTN_STRINGIFY, "&Stringify"),
        (IDC_BTN_UNESCAPE, "&Unescape"),
        (0, ""),
        (IDC_BTN_J2P, "Convert JSON to &Python"),
        (IDC_BTN_P2J, "Convert Python to &JSON"),
        (0, ""),
        (IDM_COPY_TEXT, "Copy &Text"),
        (IDM_COPY_BEAUTIFIED, "Copy Beautified JSON"),
        (IDM_COPY_MINIFIED, "Copy Minified JSON"),
        (IDM_COPY_STRINGIFIED, "Copy Stringified JSON"),
        (IDM_COPY_PYTHON, "Copy as Python Dictionary\tCtrl+Alt+C"),
        (0, ""),
        (IDC_BTN_CLEAR, "&Clear\tCtrl+K"),
    ]), "&Edit");
    add_popup(bar, popup(&[
        (IDC_TAB_VIEWER, "&Viewer Tab\tCtrl+1"),
        (IDC_TAB_TEXT, "&Text Tab\tCtrl+2"),
        (IDC_TAB_SPLIT, "&Split Tab\tCtrl+3"),
        (0, ""),
        (IDC_BTN_EXPAND, "&Expand All\tCtrl+E"),
        (IDC_BTN_COLLAPSE, "&Collapse All\tCtrl+Shift+E"),
        (IDM_TOGGLE_PROPS, "Toggle &Properties Panel\tCtrl+Alt+P"),
        (0, ""),
        (IDM_ZOOM_IN, "Zoom &In\tCtrl++"),
        (IDM_ZOOM_OUT, "Zoom &Out\tCtrl+-"),
        (IDM_ZOOM_RESET, "&Reset Zoom\tCtrl+0"),
    ]), "&View");
    add_popup(bar, popup(&[
        (IDM_FIND, "&Find...\tCtrl+F"),
        (IDM_FIND_NEXT, "Find &Next\tCtrl+G"),
        (IDM_FIND_PREV, "Find &Previous\tCtrl+Shift+G"),
    ]), "&Search");
    add_popup(bar, popup(&[
        (IDM_SETTINGS, "&Settings...\tCtrl+,"),
        (IDM_SHORTCUTS, "&Keyboard Shortcuts"),
        (IDM_ABOUT, "&About JSON Viewer"),
    ]), "&Help");
    bar
}

fn build_accelerators() -> windows::Win32::UI::WindowsAndMessaging::HACCEL {
    unsafe {
        let e = |key: u16, cmd: u32, shift: bool| ACCEL {
            fVirt: windows::Win32::UI::WindowsAndMessaging::ACCEL_VIRT_FLAGS(
                FVIRTKEY.0 as u8 | FCONTROL.0 as u8 | if shift { FSHIFT.0 as u8 } else { 0 },
            ),
            key,
            cmd: cmd as u16,
        };
        let e_alt = |key: u16, cmd: u32| ACCEL {
            fVirt: windows::Win32::UI::WindowsAndMessaging::ACCEL_VIRT_FLAGS(
                FVIRTKEY.0 as u8 | FCONTROL.0 as u8 | FALT.0 as u8,
            ),
            key,
            cmd: cmd as u16,
        };
        let table = [
            e(0x4F, IDC_BTN_OPEN, false),   // Ctrl+O
            e(0x53, IDC_BTN_SAVE, false),   // Ctrl+S
            e(0x46, IDM_FIND, false),       // Ctrl+F
            e(0x47, IDM_FIND_NEXT, false),  // Ctrl+G
            e(0x47, IDM_FIND_PREV, true),   // Ctrl+Shift+G
            e(0x4B, IDC_BTN_CLEAR, false),  // Ctrl+K
            e(0x45, IDC_BTN_EXPAND, false), // Ctrl+E
            e(0x45, IDC_BTN_COLLAPSE, true),
            e(0x31, IDC_TAB_VIEWER, false),
            e(0x32, IDC_TAB_TEXT, false),
            e(0x33, IDC_TAB_SPLIT, false),
            e(0x30, IDM_ZOOM_RESET, false),
            e(0xBB, IDM_ZOOM_IN, false),    // Ctrl+= (plus)
            e(0xBD, IDM_ZOOM_OUT, false),   // Ctrl+-
            e(0xBC, IDM_SETTINGS, false),   // Ctrl+, (VK_OEM_COMMA)
            e_alt(0x43, IDM_COPY_PYTHON),   // Ctrl+Alt+C
            e_alt(0x50, IDM_TOGGLE_PROPS),  // Ctrl+Alt+P
            e(0x43, IDM_ACCEL_COPY, false),      // Ctrl+C (focus-aware)
            e(0x56, IDM_ACCEL_PASTE, false),     // Ctrl+V (smart paste)
            e(0x58, IDM_ACCEL_CUT, false),       // Ctrl+X
            e(0x41, IDM_ACCEL_SELECTALL, false), // Ctrl+A
        ];
        CreateAcceleratorTableW(&table).unwrap_or_default()
    }
}

// ---------------- search edit subclass ----------------

extern "system" fn search_edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        const WM_KEYDOWN_LOCAL: u32 = 0x0100;
        if msg == WM_KEYDOWN_LOCAL {
            let vk = (wparam.0 & 0xffff) as u32;
            if vk == 0x0D {
                // Enter / Shift+Enter. Reached via loop Dispatch with no App
                // borrow live, so borrowing here is safe.
                let shift = GetKeyState(0x10) < 0;
                if APP_PTR != 0 {
                    let main = HWND(APP_PTR as *mut std::ffi::c_void);
                    if let Some(app) = app_of(main) {
                        do_search(app, if shift { -1 } else { 1 });
                    }
                }
                return LRESULT(0);
            } else if vk == 0x1B {
                if APP_PTR != 0 {
                    let main = HWND(APP_PTR as *mut std::ffi::c_void);
                    if let Some(app) = app_of(main) {
                        clear_search_ui(app);
                    }
                }
                return LRESULT(0);
            }
        } else if msg == WM_CHAR {
            // Swallow Return/Escape chars: KEYDOWN above already ran the
            // search / clear. Letting '\r' through makes the single-line
            // EDIT beep after every successful Enter search.
            let ch = (wparam.0 & 0xffff) as u32;
            if ch == 0x0D || ch == 0x0A || ch == 0x1B {
                return LRESULT(0);
            }
        }
        // Pass-through (including WM_GETTEXT while wnd_proc holds &mut App):
        // use the global orig proc, never app_of().
        let orig = SEARCH_ORIG_PROC.load(std::sync::atomic::Ordering::Relaxed);
        if orig != 0 {
            let p: WNDPROC = std::mem::transmute(orig);
            CallWindowProcW(p, hwnd, msg, wparam, lparam)
        } else {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

// ---------------- window procedure ----------------

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CREATE => {
                let cs = lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
                let params = (*cs).lpCreateParams as *mut CreateParams;
                let params = Box::from_raw(params);
                if !init_app(hwnd, params.settings.clone(), params.store.clone(), params.initial_file.clone()) {
                    return LRESULT(-1);
                }
                LRESULT(0)
            }
            WM_SIZE => {
                if let Some(app) = app_of(hwnd) {
                    layout(app);
                }
                LRESULT(0)
            }
            // Keep the two toolbar bars usable: never shrink below their width.
            0x0024 => {
                let mmi = lparam.0 as *mut MINMAXINFO;
                if !mmi.is_null() {
                    (*mmi).ptMinTrackSize.x = 940;
                    (*mmi).ptMinTrackSize.y = 620;
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xffff) as u32;
                let code = ((wparam.0 >> 16) & 0xffff) as u32;
                let ctrl_hwnd = HWND(lparam.0 as *mut std::ffi::c_void);
                if let Some(app) = app_of(hwnd) {
                    // Editor text change notifications.
                    // IMPORTANT: keep this O(1) — just flag dirty and restart the
                    // idle timer. Pulling the full text / re-parsing here on every
                    // keystroke is what froze typing on large files.
                    if ctrl_hwnd == app.ctrl.editor && code == EN_CHANGE {
                        if !app.syncing_editor {
                            app.model.is_dirty = true;
                            // Split keeps the last successfully parsed tree on
                            // screen; flag it immediately so the status bar
                            // warns before the debounced re-parse runs.
                            if matches!(app.model.active_tab, AppTab::Split)
                                && app.model.root.is_some()
                            {
                                app.tree_stale = true;
                                refresh_status(app);
                            }
                            let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(hwnd, TIMER_LIVE_PARSE);
                            let _ = windows::Win32::UI::WindowsAndMessaging::SetTimer(hwnd, TIMER_LIVE_PARSE, 500, None);
                        }
                        return LRESULT(0);
                    }
                    if ctrl_hwnd == app.ctrl.search_edit && code == EN_CHANGE {
                        app.model.search_query = get_window_text(app.ctrl.search_edit);
                        // Emptying the query clears results immediately, like
                        // the macOS/Linux search bars, and returns the tree
                        // viewport to the root.
                        if app.model.search_query.trim().is_empty() {
                            app.model.clear_search();
                            clear_search_marks_and_repaint(app);
                            stop_search_poll(app);
                            scroll_tree_to_root(app);
                            refresh_status(app);
                        }
                        return LRESULT(0);
                    }
                    if ctrl_hwnd == app.ctrl.prop_filter && code == EN_CHANGE {
                        if !app.syncing_filter {
                            let text = get_window_text(app.ctrl.prop_filter);
                            if text != app.prop_filter {
                                app.prop_filter = text;
                                refresh_grid(app);
                                refresh_status(app);
                            }
                        }
                        return LRESULT(0);
                    }
                    if code == BN_CLICKED || lparam.0 == 0 {
                        // Button click or menu/accelerator.
                        handle_command(app, id);
                        return LRESULT(0);
                    }
                }
                LRESULT(0)
            }
            WM_NOTIFY => {
                if let Some(app) = app_of(hwnd) {
                    let hdr = &*(lparam.0 as *const windows::Win32::UI::Controls::NMHDR);
                    // Dark-theme selection for the tree: paint the selected
                    // row (current search match / inspected node) with the
                    // accent background in every state — focused or not — so
                    // the active match stays visibly highlighted while the
                    // search box has focus. Without this the selection uses
                    // the system white wash (or vanishes unfocused).
                    if hdr.hwndFrom == app.ctrl.tree && hdr.code == NM_CUSTOMDRAW {
                        let cd = &mut *(lparam.0 as *mut NMTVCUSTOMDRAW);
                        if cd.nmcd.dwDrawStage == CDDS_PREPAINT {
                            return LRESULT(CDRF_NOTIFYITEMDRAW as isize);
                        } else if cd.nmcd.dwDrawStage == CDDS_ITEMPREPAINT {
                            if (cd.nmcd.uItemState.0 & CDIS_SELECTED.0) != 0 {
                                cd.clrText = COLORREF(DARK_TEXT);
                                cd.clrTextBk = COLORREF(DARK_ACCENT_BG);
                                return LRESULT(CDRF_NEWFONT as isize);
                            }
                            // Search highlight (mac MATCH parity). Two layers:
                            // 1. The model-selected ID always gets the brown
                            //    selection background — even if the TreeView
                            //    caret itself didn't follow (stale caret after
                            //    chunked fills / programmatic jumps).
                            // 2. Other matches get bright yellow text.
                            let item = cd.nmcd.dwItemSpec as isize;
                            if let Some(id) = app.item_to_id.get(&item) {
                                if app.model.selected_id.as_ref() == Some(id) {
                                    cd.clrText = COLORREF(DARK_TEXT);
                                    cd.clrTextBk = COLORREF(DARK_ACCENT_BG);
                                    return LRESULT(CDRF_NEWFONT as isize);
                                }
                                if app.search_marks.contains(id) {
                                    cd.clrText = COLORREF(SEARCH_MATCH_TEXT);
                                    cd.clrTextBk = COLORREF(DARK_EDIT_BG);
                                    return LRESULT(CDRF_NEWFONT as isize);
                                }
                            }
                            // Type colours matching the macOS tree badges.
                            if let Some(t) = app.item_type.get(&item) {
                                cd.clrText = COLORREF(type_color(t));
                                cd.clrTextBk = COLORREF(DARK_EDIT_BG);
                                return LRESULT(CDRF_NEWFONT as isize);
                            }
                            return LRESULT(CDRF_DODEFAULT as isize);
                        }
                        return LRESULT(CDRF_DODEFAULT as isize);
                    }
                    if hdr.hwndFrom == app.ctrl.grid && hdr.code == NM_CUSTOMDRAW {
                        let cd = &mut *(lparam.0 as *mut NMLVCUSTOMDRAW);
                        let stage = cd.nmcd.dwDrawStage.0;
                        if stage == CDDS_PREPAINT.0 {
                            return LRESULT(CDRF_NOTIFYITEMDRAW as isize);
                        }
                        if stage == CDDS_ITEMPREPAINT.0 {
                            return LRESULT(CDRF_NOTIFYSUBITEMDRAW as isize);
                        }
                        if stage == (CDDS_ITEMPREPAINT.0 | CDDS_SUBITEM.0) {
                            if let Some(row) = app.grid_rows.get(cd.nmcd.dwItemSpec as usize) {
                                cd.clrText = COLORREF(match cd.iSubItem {
                                    0 => 0x00FEDC9C, // property name #9CDCFE
                                    1 => type_color(&row.typ),
                                    2 => DARK_DIM_TEXT,
                                    _ => DARK_TEXT,
                                });
                            }
                            return LRESULT(CDRF_NEWFONT as isize);
                        }
                        return LRESULT(CDRF_DODEFAULT as isize);
                    }
                    if hdr.hwndFrom == app.ctrl.tree && hdr.code == TVN_SELCHANGEDW {
                        let info = &*(lparam.0 as *const NMTREEVIEWW);
                        let item = info.itemNew.hItem.0 as isize;
                        if let Some(id) = app.item_to_id.get(&item).cloned() {
                            app.model.set_selected(id);
                            refresh_grid(app);
                            refresh_expand_panel(app);
                            refresh_status(app);
                            layout(app);
                        }
                        return LRESULT(0);
                    }
                    // Track the user's expanded branches so a Split-mode
                    // rebuild can restore them (macOS parity).
                    if hdr.hwndFrom == app.ctrl.tree && hdr.code == TVN_ITEMEXPANDEDW {
                        let info = &*(lparam.0 as *const NMTREEVIEWW);
                        let item = info.itemNew.hItem.0 as isize;
                        if let Some(id) = app.item_to_id.get(&item).cloned() {
                            if info.action == TVE_EXPAND {
                                app.expanded_ids.insert(id);
                            } else if info.action == TVE_COLLAPSE {
                                app.expanded_ids.remove(&id);
                            }
                        }
                        return LRESULT(0);
                    }
                    if hdr.hwndFrom == app.ctrl.grid && (hdr.code == LVN_ITEMCHANGED || hdr.code == NM_CLICK) {
                        if app.syncing_grid {
                            return LRESULT(0);
                        }
                        let idx = send(app.ctrl.grid, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as isize) as i32;
                        if idx >= 0 {
                            if let Some(row) = app.grid_rows.get(idx as usize) {
                                let target = row.id.clone();
                                app.model.set_selected(target.clone());
                                reveal_id(app, &target, true);
                                refresh_grid(app);
                                // Rebuilding deletes/recreates every row, which
                                // drops the selection — and a lost selection is
                                // what broke double-click-to-expand (the second
                                // click landed on a selection-less list, so the
                                // expand lookup found nothing). Restore it.
                                if let Some(pos) =
                                    app.grid_rows.iter().position(|r| r.id == target)
                                {
                                    let mut lv = LVITEMW::default();
                                    lv.state = LVIS_SELECTED;
                                    lv.stateMask = LVIS_SELECTED;
                                    app.syncing_grid = true;
                                    send(
                                        app.ctrl.grid,
                                        LVM_SETITEMSTATE,
                                        pos,
                                        &lv as *const _ as isize,
                                    );
                                    app.syncing_grid = false;
                                }
                                refresh_expand_panel(app);
                                refresh_status(app);
                                layout(app);
                                ensure_selected_visible(app);
                            }
                        }
                        return LRESULT(0);
                    }
                    // Double-click a tree leaf with long text toggles the inline
                    // expand panel below the tree (macOS inline parity — no popup).
                    if hdr.hwndFrom == app.ctrl.tree && hdr.code == windows::Win32::UI::Controls::NM_DBLCLK {
                        let caret = send(app.ctrl.tree, TVM_GETNEXTITEM, TVGN_CARET as usize, 0);
                        if caret != 0 {
                            if let Some(id) = app.item_to_id.get(&(caret as isize)).cloned() {
                                if let Some(n) = app.model.find_node(&id) {
                                    if let crate::json::JSONValue::Str(_) = &n.value {
                                        if n.is_long_text() {
                                            app.model.set_selected(id.clone());
                                            refresh_grid(app);
                                            refresh_status(app);
                                            toggle_expand_inline(app, id);
                                        }
                                    }
                                }
                            }
                        }
                        return LRESULT(0);
                    }
                    // Double-click a grid row: long values expand inline below
                    // the tree; normal rows jump to the tree node
                    // (parity with macOS click-to-jump).
                    if hdr.hwndFrom == app.ctrl.grid && hdr.code == windows::Win32::UI::Controls::NM_DBLCLK {
                        // Use the clicked row from NMITEMACTIVATE, not the
                        // current selection: single-click rebuilds the grid
                        // (dropping selection), so GETNEXTITEM is unreliable
                        // here — this is why Expand previously never opened.
                        let nm = &*(lparam.0 as *const NMITEMACTIVATE);
                        let idx = nm.iItem;
                        if idx >= 0 {
                            if let Some(row) = app.grid_rows.get(idx as usize).cloned() {
                                let mut expanded = false;
                                if let Some(n) = app.model.find_node(&row.id) {
                                    if let crate::json::JSONValue::Str(s) = &n.value {
                                        if n.is_long_text() || s.chars().count() > 200 {
                                            app.model.set_selected(row.id.clone());
                                            reveal_id(app, &row.id, true);
                                            refresh_grid(app);
                                            refresh_status(app);
                                            show_expand_inline(app, row.id.clone());
                                            expanded = true;
                                        }
                                    }
                                }
                                if !expanded {
                                    // Focus the tree after jumping.
                                    reveal_id(app, &row.id, true);
                                    refresh_expand_panel(app);
                                    refresh_status(app);
                                    layout(app);
                                    ensure_selected_visible(app);
                                    let _ = SetFocus(app.ctrl.tree);
                                } else {
                                    // show_expand_inline already laid out; make
                                    // sure the just-expanded node stays in view.
                                    ensure_selected_visible(app);
                                }
                            }
                        }
                        return LRESULT(0);
                    }
                }
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == TIMER_FILL {
                    // Chunked tree fill: insert the next slice, then yield so
                    // typing, scrolling and resizing stay smooth.
                    if let Some(app) = app_of(hwnd) {
                        fill_tick(app);
                    }
                } else if wparam.0 == TIMER_SEARCH {
                    if let Some(app) = app_of(hwnd) {
                        apply_search_completion(app);
                    }
                } else if wparam.0 == TIMER_LIVE_PARSE {
                    let _ = windows::Win32::UI::WindowsAndMessaging::KillTimer(hwnd, TIMER_LIVE_PARSE);
                    if let Some(app) = app_of(hwnd) {
                        // Idle sync: pull typed text + refresh counts (cheap),
                        // re-parse only when the tree is actually showing.
                        if pull_editor_text(app) {
                            refresh_status(app);
                        }
                        // Large files: skip auto re-parse in Split. Parsing +
                        // flattening 1MB+ on the UI thread (≈300ms for 5MB)
                        // freezes typing and makes the keyboard feel stuck.
                        // The user refreshes explicitly via Refresh/tab-switch.
                        let big = app.model.raw_text.len() > LARGE_FILE_BYTES;
                        if matches!(app.model.active_tab, AppTab::Split)
                            && (app.model.root.is_none() || app.model.is_dirty)
                            && !big
                        {
                            parse_and_view(app, true);
                        } else if big && app.model.is_dirty {
                            refresh_status(app);
                        }
                    }
                }
                LRESULT(0)
            }
            WM_DRAWITEM => {
                // Owner-drawn dark toolbar buttons.
                let di = &*(lparam.0 as *const DRAWITEMSTRUCT);
                if let Some(app) = app_of(hwnd) {
                    draw_button(app, di);
                }
                LRESULT(1)
            }
            WM_CTLCOLOREDIT => {
                // Dark editor + search box.
                if let Some(app) = app_of(hwnd) {
                    let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut std::ffi::c_void);
                    SetTextColor(hdc, COLORREF(DARK_TEXT));
                    SetBkColor(hdc, COLORREF(DARK_EDIT_BG));
                    SetBkMode(hdc, OPAQUE);
                    return LRESULT(app.brush_edit.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CTLCOLORSTATIC => {
                // Dark labels / status bars (transparent over the chrome bg).
                if let Some(app) = app_of(hwnd) {
                    let hdc = windows::Win32::Graphics::Gdi::HDC(wparam.0 as *mut std::ffi::c_void);
                    SetTextColor(hdc, COLORREF(DARK_TEXT));
                    SetBkMode(hdc, TRANSPARENT);
                    return LRESULT(app.brush_chrome.0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CONTEXTMENU => {
                if let Some(app) = app_of(hwnd) {
                    if wparam.0 == app.ctrl.tree.0 as usize {
                        show_tree_context_menu(app, lparam.0 as isize);
                        return LRESULT(0);
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CHAR => {
                // Quick keys '/' focuses search, '?' opens shortcuts — unless typing in an edit.
                let ch = (wparam.0 & 0xffff) as u32;
                if ch == '/' as u32 || ch == '?' as u32 {
                    if let Some(app) = app_of(hwnd) {
                        let focus = GetFocus();
                        if focus != app.ctrl.editor && focus != app.ctrl.search_edit {
                            if ch == '/' as u32 {
                                if matches!(app.model.active_tab, AppTab::Text) {
                                    set_tab(app, AppTab::Split);
                                }
                                let _ = SetFocus(app.ctrl.search_edit);
                            } else {
                                show_shortcuts(hwnd);
                            }
                            return LRESULT(0);
                        }
                    }
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DROPFILES => {
                let hdrop = HDROP(wparam.0 as *mut std::ffi::c_void);
                let len = DragQueryFileW(hdrop, 0, None);
                if len > 0 {
                    let mut buf = vec![0u16; len as usize + 1];
                    DragQueryFileW(hdrop, 0, Some(&mut buf));
                    let path = std::path::PathBuf::from(String::from_utf16_lossy(
                        &buf[..len as usize],
                    ));
                    if let Some(app) = app_of(hwnd) {
                        let settings = app.settings.lock().unwrap().clone();
                        match app.model.load_file(&path, &settings) {
                            Ok(_) => {
                                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                    set_window_text(hwnd, &format!("JSON Viewer — {}", name));
                                }
                                app.expanded_ids.clear();
                                app.tree_stale = false;
                                refresh_editor(app);
                                refill_tree_after_model_change(app, false);
                            }
                            Err(e) => show_error(hwnd, &e),
                        }
                    }
                }
                DragFinish(hdrop);
                LRESULT(0)
            }
            WM_APP_SETTINGS_CHANGED => {
                if let Some(app) = app_of(hwnd) {
                    save_settings(app);
                    rebuild_font(app);
                    apply_wrap_style(app);
                    layout(app);
                    refresh_status(app);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
                if !ptr.is_null() {
                    let app = Box::from_raw(ptr);
                    if !app.font_mono.is_invalid() {
                        let _ = DeleteObject(app.font_mono);
                    }
                    if !app.font_ui.is_invalid() {
                        let _ = DeleteObject(app.font_ui);
                    }
                    if !app.brush_edit.is_invalid() {
                        let _ = DeleteObject(app.brush_edit);
                    }
                    if !app.brush_chrome.is_invalid() {
                        let _ = DeleteObject(app.brush_chrome);
                    }
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                APP_PTR = 0;
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// "Copy" split-button popup (mirrors the macOS Copy menu): Text, Beautified,
/// Minified, Stringified, Python Dictionary.
fn show_copy_menu(app: &App) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or(HMENU(std::ptr::null_mut()));
        if menu.is_invalid() {
            return;
        }
        for (id, label) in [
            (IDM_COPY_TEXT, "Copy Text"),
            (IDM_COPY_BEAUTIFIED, "Copy Beautified JSON"),
            (IDM_COPY_MINIFIED, "Copy Minified JSON"),
            (IDM_COPY_STRINGIFIED, "Copy as Stringified JSON"),
            (IDM_COPY_PYTHON, "Copy as Python Dictionary"),
        ] {
            let w = wide(label);
            let _ = AppendMenuW(menu, MF_STRING, id as usize, windows::core::PCWSTR::from_raw(w.as_ptr()));
        }
        // Drop the menu under the Copy button (fallback: cursor position).
        let (x, y) = {
            let mut anchor = POINT::default();
            let mut found = false;
            for item in app.ctrl.row_text.iter().chain(app.ctrl.row_viewer.iter()) {
                if item.id == IDC_BTN_COPY && !item.hwnd.is_invalid() {
                    let mut r = RECT::default();
                    if GetWindowRect(item.hwnd, &mut r).is_ok() {
                        anchor.x = r.left;
                        anchor.y = r.bottom;
                        found = true;
                    }
                    break;
                }
            }
            if found {
                (anchor.x, anchor.y)
            } else {
                let mut pt = POINT::default();
                let _ = GetCursorPos(&mut pt);
                (pt.x, pt.y)
            }
        };
        let _ = SetForegroundWindow(app.hwnd);
        let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON, x, y, 0, app.hwnd, None);
        let _ = DestroyMenu(menu);
    }
}

/// Right-click menu on a tree node (mac parity: Expand/Collapse, Copy Value,
/// Copy Key, Copy Path, Copy Subtree, Copy as Python, Expand Full Text inline).
fn show_tree_context_menu(app: &mut App, lparam: isize) {
    unsafe {
        let tree = app.ctrl.tree;
        // Keyboard-invoked menus arrive with x == y == -1: use the caret.
        let mut screen_x = (lparam & 0xffff) as i32;
        let mut screen_y = ((lparam >> 16) & 0xffff) as i32;
        let item: isize = if screen_x == -1 && screen_y == -1 {
            send(tree, TVM_GETNEXTITEM, TVGN_CARET as usize, 0)
        } else {
            let pt = POINT { x: screen_x, y: screen_y };
            let mut client = pt;
            let _ = ScreenToClient(tree, &mut client);
            let mut hit = TVHITTESTINFO {
                pt: client,
                flags: windows::Win32::UI::Controls::TVHITTESTINFO_FLAGS(0),
                hItem: windows::Win32::UI::Controls::HTREEITEM(0),
            };
            let found = send(tree, TVM_HITTEST, 0, &mut hit as *mut _ as isize);
            if found == 0 || (hit.flags.0 & TVHT_ONITEM.0) == 0 {
                return;
            }
            screen_x = pt.x;
            screen_y = pt.y;
            found
        };
        if item == 0 {
            return;
        }
        let id = match app.item_to_id.get(&item).cloned() {
            Some(id) => id,
            None => return,
        };
        // Select what was right-clicked (mac behavior).
        send(tree, TVM_SELECTITEM, TVGN_CARET as usize, item);
        app.model.set_selected(id.clone());
        refresh_grid(app);
        refresh_expand_panel(app);
        refresh_status(app);
        layout(app);
        app.ctx_id = Some(id.clone());

        let is_container = app
            .model
            .find_node(&id)
            .map(|n| n.is_container())
            .unwrap_or(false);
        let is_long = app
            .model
            .find_node(&id)
            .map(|n| n.is_long_text())
            .unwrap_or(false);

        let menu = CreatePopupMenu().unwrap_or(HMENU(std::ptr::null_mut()));
        if menu.is_invalid() {
            return;
        }
        let mut entries: Vec<(u32, &str)> = Vec::new();
        if is_container {
            entries.push((IDM_CTX_EXPAND_SUB, "Expand All Sub-levels"));
            entries.push((IDM_CTX_COLLAPSE, "Collapse"));
            entries.push((0, ""));
        } else if is_long {
            // Inline (no "…" — no separate dialog opens anymore).
            let label = if app.expand_visible && app.expand_id.as_ref() == Some(&id) {
                "Collapse Full Text"
            } else {
                "Expand Full Text"
            };
            entries.push((IDM_CTX_EXPAND_TEXT, label));
            entries.push((0, ""));
        }
        entries.push((IDM_CTX_COPY_VALUE, "Copy Value"));
        entries.push((IDM_CTX_COPY_KEY, "Copy Key"));
        entries.push((IDM_CTX_COPY_PATH, "Copy JSON Path"));
        entries.push((IDM_CTX_COPY_SUBTREE, "Copy Subtree as JSON"));
        entries.push((IDM_CTX_COPY_PYTHON, "Copy as Python Dictionary"));
        for (id, label) in entries {
            if id == 0 {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            } else {
                let w = wide(label);
                let _ = AppendMenuW(menu, MF_STRING, id as usize, windows::core::PCWSTR::from_raw(w.as_ptr()));
            }
        }
        let _ = SetForegroundWindow(app.hwnd);
        let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON, screen_x, screen_y, 0, app.hwnd, None);
        let _ = DestroyMenu(menu);
    }
}

/// Expand all descendants of a tree item (used by the context menu).
fn expand_subtree_items(tree: HWND, item: isize) {
    set_redraw(tree, false);
    let mut stack = vec![item];
    while let Some(cur) = stack.pop() {
        tree_expand_by_hwnd(tree, cur, true);
        // Enumerate children.
        let mut child = send(tree, TVM_GETNEXTITEM, TVGN_CHILD as usize, cur);
        while child != 0 {
            stack.push(child);
            child = send(tree, TVM_GETNEXTITEM, TVGN_NEXT as usize, child);
        }
    }
    set_redraw(tree, true);
}

/// Copy helpers for the tree context menu (always act on the right-clicked
/// node stored in `ctx_id`).
fn ctx_copy(app: &mut App, kind: &str) {
    let id = match app.ctx_id.clone() {
        Some(id) => id,
        None => return,
    };
    let node = match app.model.find_node(&id) {
        Some(n) => n.clone(),
        None => return,
    };
    let settings = app.settings.lock().unwrap().clone();
    let (text, label) = match kind {
        "key" => (node.key.clone(), "Copied Key!"),
        "path" => (node.path.clone(), "Copied JSON Path!"),
        "subtree" => (
            node.value.format(settings.indent_spaces, settings.sort_keys_alphabetically, false),
            "Copied Subtree as JSON!",
        ),
        "python" => {
            let indent = if settings.indent_spaces < 0 { 4 } else { settings.indent_spaces as usize };
            (node.value.to_python_object(indent), "Copied Python Dictionary!")
        }
        _ => (node.formatted_value_string(), "Copied Value!"),
    };
    if clipboard::set_text(&text) {
        unsafe {
            set_window_text(app.ctrl.status, label);
        }
    }
}

/// The edit control with keyboard focus, if any (typing targets).
fn focused_edit(app: &App) -> Option<HWND> {
    unsafe {
        let f = GetFocus();
        if f == app.ctrl.editor || f == app.ctrl.search_edit {
            Some(f)
        } else {
            None
        }
    }
}

/// Copy/cut/select-all targets: typing edits plus the read-only inline
/// expand viewer (so Ctrl+C copies the selection there natively instead of
/// firing the global subtree copy).
fn focused_copyable_edit(app: &App) -> Option<HWND> {
    unsafe {
        let f = GetFocus();
        if f == app.ctrl.editor || f == app.ctrl.search_edit || f == app.ctrl.expand_text {
            Some(f)
        } else {
            None
        }
    }
}

/// Ctrl+C: native selection copy inside edits; subtree copy on the tree;
/// selected grid rows on the grid; full text everywhere else.
fn accel_copy(app: &mut App) {
    unsafe {
        if let Some(f) = focused_copyable_edit(app) {
            let _ = SendMessageW(f, WM_COPY, WPARAM(0), LPARAM(0));
            return;
        }
        let focus = GetFocus();
        if focus == app.ctrl.tree {
            if let Some(id) = app.model.selected_id.clone() {
                if let Some(n) = app.model.find_node(&id) {
                    let t = n.value.format(2, false, false);
                    if clipboard::set_text(&t) {
                        set_window_text(app.ctrl.status, "Copied Subtree as JSON!");
                    }
                    return;
                }
            }
        }
        if focus == app.ctrl.grid {
            let mut lines: Vec<String> = Vec::new();
            let mut idx = send(app.ctrl.grid, LVM_GETNEXTITEM, (-1isize) as usize, LVNI_SELECTED as isize);
            while idx >= 0 {
                if let Some(row) = app.grid_rows.get(idx as usize) {
                    lines.push(format!("{}: {}", row.name, row.value));
                }
                idx = send(app.ctrl.grid, LVM_GETNEXTITEM, idx as usize, LVNI_SELECTED as isize);
            }
            if !lines.is_empty() {
                if clipboard::set_text(&lines.join("\r\n")) {
                    set_window_text(app.ctrl.status, "Copied Rows!");
                }
                return;
            }
        }
        do_copy(app, "text");
    }
}

/// Ctrl+V: smart paste at the caret inside the editor (Python dicts are
/// auto-converted, like the mac app); the Paste-button behavior elsewhere.
fn accel_paste(app: &mut App) {
    unsafe {
        // Read-only inline viewer: fall through to global paste (replace
        // document), like tree/grid focus — never EM_REPLACESEL into it.
        if GetFocus() == app.ctrl.expand_text {
            do_paste(app);
            return;
        }
        if let Some(f) = focused_edit(app) {
            if f == app.ctrl.search_edit {
                let _ = SendMessageW(f, WM_PASTE, WPARAM(0), LPARAM(0));
                return;
            }
            if let Some(text) = clipboard::get_text() {
                let trimmed = text.trim();
                let converted = if JSONParser::parse(trimmed).is_err() {
                    parse_python_literal(trimmed).ok().map(|v| {
                        let s = app.settings.lock().unwrap().clone();
                        v.format(s.indent_spaces, s.sort_keys_alphabetically, s.escape_slashes_in_stringify)
                    })
                } else {
                    None
                };
                let insert = converted.as_deref().unwrap_or(&text);
                let w: Vec<u16> = insert.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = SendMessageW(f, EM_REPLACESEL, WPARAM(1), LPARAM(w.as_ptr() as isize));
                if converted.is_some() {
                    set_window_text(app.ctrl.status, "Converted Python Dictionary to JSON!");
                }
            }
            return;
        }
        do_paste(app);
    }
}

/// Ctrl+X: native cut inside edits; otherwise behaves like copy.
fn accel_cut(app: &mut App) {
    unsafe {
        if let Some(f) = focused_copyable_edit(app) {
            let _ = SendMessageW(f, WM_CUT, WPARAM(0), LPARAM(0));
            return;
        }
    }
    accel_copy(app);
}

/// Ctrl+A: select all in edits and the grid.
fn accel_select_all(app: &mut App) {
    unsafe {
        if let Some(f) = focused_copyable_edit(app) {
            let _ = SendMessageW(f, EM_SETSEL, WPARAM(0), LPARAM(-1));
            return;
        }
        if GetFocus() == app.ctrl.grid {
            let mut lv = LVITEMW::default();
            lv.state = LVIS_SELECTED;
            lv.stateMask = LVIS_SELECTED;
            send(app.ctrl.grid, LVM_SETITEMSTATE, (-1isize) as usize, &lv as *const _ as isize);
        }
    }
}

fn handle_command(app: &mut App, id: u32) {    match id {
        IDC_TAB_VIEWER => set_tab(app, AppTab::Viewer),
        IDC_TAB_TEXT => set_tab(app, AppTab::Text),
        IDC_TAB_SPLIT => set_tab(app, AppTab::Split),
        IDC_BTN_FORMAT => do_transform(app, "format"),
        IDC_BTN_MINIFY => do_transform(app, "minify"),
        IDC_BTN_STRINGIFY => do_stringify(app),
        IDC_BTN_UNESCAPE => do_unescape(app),
        IDC_BTN_J2P => do_transform(app, "j2p"),
        IDC_BTN_P2J => do_transform(app, "p2j"),
        IDC_BTN_COPY => show_copy_menu(app),
        IDM_COPY_TEXT => do_copy(app, "text"),
        IDM_COPY_BEAUTIFIED => do_copy(app, "beautified"),
        IDM_COPY_MINIFIED => do_copy(app, "minified"),
        IDM_COPY_STRINGIFIED => do_copy(app, "stringified"),
        IDM_COPY_PYTHON => do_copy(app, "python"),
        IDC_BTN_PASTE => do_paste(app),
        IDC_BTN_CLEAR => {
            app.model.clear();
            app.expanded_ids.clear();
            app.tree_stale = false;
            stop_search_poll(app);
            refresh_editor(app);
            refresh_status(app);
            begin_tree_fill(app, false);
        }
        IDC_BTN_EXPAND => {
            let _ = parse_and_view(app, true);
            if app.fill.is_some() {
                // Big tree still loading: expand everything when it lands.
                app.expand_on_fill_done = true;
                unsafe {
                    set_window_text(app.ctrl.status, "Loading tree… will expand all when done");
                }
            } else {
                expand_all(app);
            }
        }
        IDC_BTN_COLLAPSE => collapse_all(app),
        IDC_BTN_OPEN => do_open(app),
        IDC_BTN_SAVE => do_save(app),
        IDC_BTN_PARSE => {
            parse_and_view(app, false);
        }
        IDC_SEARCH_GO => do_search(app, 0),
        IDC_SEARCH_NEXT => do_search(app, 1),
        IDC_SEARCH_PREV => do_search(app, -1),
        IDM_FIND => unsafe {
            if matches!(app.model.active_tab, AppTab::Text) {
                set_tab(app, AppTab::Split);
            }
            let _ = SetFocus(app.ctrl.search_edit);
        },
        IDM_FIND_NEXT => do_search(app, 1),
        IDM_FIND_PREV => do_search(app, -1),
        IDM_TOGGLE_PROPS => {
            app.props_visible = !app.props_visible;
            layout(app);
        }
        IDC_BTN_PROPS => {
            app.props_visible = !app.props_visible;
            layout(app);
        }
        IDC_PROPS_PARENT => {
            // macOS "← Parent" breadcrumb jump.
            if let Some(parent) = app.model.parent_of_selected() {
                app.model.set_selected(parent.id.clone());
                reveal_selected(app, true);
                refresh_status(app);
            }
        }
        IDC_BTN_FIND => unsafe {
            if matches!(app.model.active_tab, AppTab::Text) {
                set_tab(app, AppTab::Split);
            }
            let _ = SetFocus(app.ctrl.search_edit);
        },
        IDC_BTN_SHORTCUTS => show_shortcuts(app.hwnd),
        IDC_BTN_SETTINGS => open_settings_dialog(app),
        IDM_ZOOM_IN => do_zoom(app, 1),
        IDM_ZOOM_OUT => do_zoom(app, -1),
        IDM_ZOOM_RESET => do_zoom(app, 0),
        IDM_SETTINGS => open_settings_dialog(app),
        IDM_SHORTCUTS => show_shortcuts(app.hwnd),
        IDM_ABOUT => show_about(app.hwnd),
        IDM_ACCEL_COPY => accel_copy(app),
        IDM_ACCEL_PASTE => accel_paste(app),
        IDM_ACCEL_CUT => accel_cut(app),
        IDM_ACCEL_SELECTALL => accel_select_all(app),
        IDM_CTX_EXPAND_SUB => {
            if let Some(id) = app.ctx_id.clone() {
                if let Some(&item) = app.id_to_item.get(&id) {
                    expand_subtree_items(app.ctrl.tree, item);
                }
            }
        }
        IDM_CTX_COLLAPSE => {
            if let Some(id) = app.ctx_id.clone() {
                if let Some(&item) = app.id_to_item.get(&id) {
                    tree_expand_by_hwnd(app.ctrl.tree, item, false);
                }
            }
        }
        IDM_CTX_COPY_VALUE => ctx_copy(app, "value"),
        IDM_CTX_COPY_KEY => ctx_copy(app, "key"),
        IDM_CTX_COPY_PATH => ctx_copy(app, "path"),
        IDM_CTX_COPY_SUBTREE => ctx_copy(app, "subtree"),
        IDM_CTX_COPY_PYTHON => ctx_copy(app, "python"),
        IDM_CTX_EXPAND_TEXT => {
            // Inline toggle (macOS parity) — no separate popup window.
            if let Some(id) = app.ctx_id.clone() {
                app.model.set_selected(id.clone());
                refresh_grid(app);
                refresh_status(app);
                toggle_expand_inline(app, id);
            }
        }
        IDC_EXPAND_COPY => copy_expand_text(app),
        IDC_EXPAND_COLLAPSE => hide_expand_inline(app),
        IDM_EXIT => unsafe {
            let _ = DestroyWindow(app.hwnd);
        },
        _ => {}
    }
}

fn open_settings_dialog(app: &App) {
    let hwnd_addr = app.hwnd.0 as usize;
    let ctx = SettingsContext {
        settings: app.settings.clone(),
        on_apply: Arc::new(move || unsafe {
            let _ = PostMessageW(HWND(hwnd_addr as *mut std::ffi::c_void), WM_APP_SETTINGS_CHANGED, WPARAM(0), LPARAM(0));
        }),
    };
    settings_dialog::open_settings(ctx);
}

fn init_app(
    hwnd: HWND,
    settings: Arc<Mutex<Settings>>,
    store: Arc<Mutex<SettingsStore>>,
    initial_file: Option<std::path::PathBuf>,
) -> bool {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let current = settings.lock().unwrap().clone();
        let font_mono = make_font("Consolas", current.font_size);
        let font_ui = make_font("Segoe UI", 9.0);

        // Row 1 — macOS toolbar: tab switcher left, utilities right.
        let row_tabs = build_bar(hwnd, &[
            (IDC_TAB_VIEWER, "Viewer"),
            (IDC_TAB_TEXT, "Text"),
            (IDC_TAB_SPLIT, "Split"),
        ], font_ui, instance);
        let row_utils = build_bar(hwnd, &[
            (IDC_BTN_PROPS, "Properties"),
            (IDC_BTN_FIND, "Find"),
            (IDC_BTN_SHORTCUTS, "?"),
            (IDC_BTN_SETTINGS, "Settings"),
        ], font_ui, instance);

        // Row 2 — macOS Text-tab toolbar: clipboard | transforms | python | files.
        let row_text = build_bar(hwnd, &[
            (IDC_BTN_PASTE, "Paste"),
            (IDC_BTN_COPY, "Copy"),
            (IDC_BTN_CLEAR, "Clear"),
            (0, ""),
            (IDC_BTN_FORMAT, "Format"),
            (IDC_BTN_MINIFY, "Minify"),
            (IDC_BTN_STRINGIFY, "Stringify"),
            (IDC_BTN_UNESCAPE, "Unescape"),
            (0, ""),
            (IDC_BTN_J2P, "JSON to Python"),
            (IDC_BTN_P2J, "Python to JSON"),
            (0, ""),
            (IDC_BTN_OPEN, "Open"),
            (IDC_BTN_SAVE, "Save"),
        ], font_ui, instance);

        // Row 2 — macOS Tree toolbar: expand/collapse | copy | zoom | refresh.
        let row_viewer = build_bar(hwnd, &[
            (IDC_BTN_EXPAND, "Expand All"),
            (IDC_BTN_COLLAPSE, "Collapse All"),
            (0, ""),
            (IDC_BTN_COPY, "Copy"),
            (0, ""),
            (IDM_ZOOM_OUT, "Zoom Out"),
            (IDM_ZOOM_IN, "Zoom In"),
            (0, ""),
            (IDC_BTN_PARSE, "Refresh"),
        ], font_ui, instance);

        // Editor
        let editor = create_child(hwnd, &edit_class(), "", IDC_EDITOR,
            WS_CHILD_VISIBLE | WS_TABSTOP.0 | WS_BORDER.0 | WS_VSCROLL.0 | WS_HSCROLL.0
                | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_AUTOHSCROLL as u32 | ES_WANTRETURN as u32,
            0, instance);

        // Tree
        let tree = create_child(hwnd, &WC_TREEVIEW, "", IDC_TREE,
            WS_CHILD_VISIBLE | WS_TABSTOP.0 | WS_BORDER.0 | WS_VSCROLL.0 | WS_HSCROLL.0
                | TVS_HASLINES | TVS_LINESATROOT | TVS_HASBUTTONS | TVS_DISABLEDRAGDROP,
            windows::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE.0, instance);

        // Grid (report ListView)
        let grid = create_child(hwnd, &WC_LISTVIEW, "", IDC_GRID,
            WS_CHILD_VISIBLE | WS_TABSTOP.0 | WS_BORDER.0 | WS_VSCROLL.0 | WS_HSCROLL.0
                | LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS,
            windows::Win32::UI::WindowsAndMessaging::WS_EX_CLIENTEDGE.0, instance);
        send(grid, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, (LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES) as isize);
        let cols = [("Property", 170), ("Value", 250), ("Type", 90)];
        for (i, (name, width)) in cols.iter().enumerate() {
            let w = wide(name);
            let col = LVCOLUMNW {
                mask: LVCF_FMT | LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM,
                fmt: LVCFMT_LEFT,
                cx: *width,
                pszText: windows::core::PWSTR(w.as_ptr() as *mut u16),
                iSubItem: i as i32,
                ..Default::default()
            };
            send(grid, LVM_INSERTCOLUMNW, i, &col as *const _ as isize);
        }

        // Search bar (labels vertically centered to share the text
        // baseline with the edit box and buttons in the same row).
        let search_label = make_static(hwnd, "Search:", 0, instance, SS_CENTERIMAGE);
        let search_edit = create_child(hwnd, &edit_class(), "", IDC_SEARCH_EDIT,
            WS_CHILD_VISIBLE | WS_TABSTOP.0 | WS_BORDER.0 | ES_AUTOHSCROLL as u32, 0, instance);
        let search_go = make_button(hwnd, "GO!", IDC_SEARCH_GO, instance);
        let search_prev = make_button(hwnd, "Previous", IDC_SEARCH_PREV, instance);
        let search_next = make_button(hwnd, "Next", IDC_SEARCH_NEXT, instance);
        let search_status = make_static(hwnd, "", IDC_SEARCH_STATUS, instance, SS_CENTERIMAGE);
        for h in [search_label, search_go, search_prev, search_next, search_status] {
            set_font(h, font_ui);
        }
        let search_cue = wide("Search keys, values, paths...");
        send(search_edit, EM_SETCUEBANNER, 1, search_cue.as_ptr() as isize);

        // Inline expand panel (macOS expanded big-text parity): docked below
        // the tree, inside the main window — no separate popup.
        // Word-wrap ON (no WS_HSCROLL / ES_AUTOHSCROLL) so long lines wrap
        // like the mac full-width textarea. Read-only + selectable.
        let expand_label = make_static(hwnd, "", IDC_EXPAND_LABEL, instance, SS_CENTERIMAGE);
        let expand_text = create_child(hwnd, &edit_class(), "", IDC_EXPAND_TEXT,
            WS_CHILD_VISIBLE | WS_TABSTOP.0 | WS_BORDER.0 | WS_VSCROLL.0
                | ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32 | ES_READONLY as u32,
            0, instance);
        let expand_copy = make_button(hwnd, "Copy", IDC_EXPAND_COPY, instance);
        let expand_collapse = make_button(hwnd, "Collapse", IDC_EXPAND_COLLAPSE, instance);
        for h in [expand_label, expand_copy, expand_collapse] {
            set_font(h, font_ui);
        }
        set_font(expand_text, font_mono);

        let status = make_static(hwnd, "Ready", IDC_STATUS, instance, SS_CENTERIMAGE);
        let path = make_static(hwnd, "$", IDC_PATH, instance, SS_CENTERIMAGE);

        // Property inspector header: title, parent jump, filter, item count.
        let prop_title = make_static(hwnd, "Properties", IDC_PROPS_TITLE, instance, SS_CENTERIMAGE);
        let prop_parent = make_button(hwnd, "← Parent", IDC_PROPS_PARENT, instance);
        let prop_filter_label = make_static(hwnd, "Filter:", IDC_PROPS_FILTER_LABEL, instance, SS_CENTERIMAGE);
        let prop_filter = create_child(hwnd, &edit_class(), "", IDC_PROPS_FILTER,
            WS_CHILD_VISIBLE | WS_TABSTOP.0 | WS_BORDER.0 | ES_AUTOHSCROLL as u32, 0, instance);
        let filter_cue = wide("Filter properties...");
        send(prop_filter, EM_SETCUEBANNER, 1, filter_cue.as_ptr() as isize);
        let prop_count = make_static(hwnd, "", IDC_PROPS_COUNT, instance, SS_CENTERIMAGE | 0x0002); // SS_RIGHT

        let menu = build_menu();
        let _ = SetMenu(hwnd, menu);
        let _ = DrawMenuBar(hwnd);

        let mut app = Box::new(App {
            hwnd,
            settings: settings.clone(),
            store: store.clone(),
            model: DocumentModel::new(&current),
            ctrl: Controls {
                row_tabs, row_utils, row_text, row_viewer,
                editor, tree, grid,
                expand_label, expand_text, expand_copy, expand_collapse,
                search_label, search_edit, search_go, search_prev, search_next, search_status,
                status, path,
                prop_title, prop_parent, prop_filter_label, prop_filter, prop_count,
            },
            font_mono,
            font_ui,
            brush_edit: CreateSolidBrush(COLORREF(DARK_EDIT_BG)),
            brush_chrome: CreateSolidBrush(COLORREF(DARK_CHROME_BG)),
            item_to_id: HashMap::new(),
            id_to_item: HashMap::new(),
            item_type: HashMap::new(),
            container_items: Vec::new(),
            expanded_ids: HashSet::new(),
            grid_rows: Vec::new(),
            grid_parent: None,
            grid_truncated: None,
            grid_total: 0,
            prop_filter: String::new(),
            syncing_filter: false,
            syncing_editor: false,
            syncing_grid: false,
            props_visible: true,
            search_orig_proc: 0,
            fill: None,
            fill_gen: 0,
            tree_complete: false,
            tree_stale: false,
            expand_on_fill_done: false,
            ctx_id: None,
            expand_visible: false,
            expand_id: None,
            search_marks: HashSet::new(),
        });

        if current.wrap_lines {
            // drop horizontal scroll for wrapping
            use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWL_STYLE};
            let mut style = GetWindowLongPtrW(editor, GWL_STYLE) as u32;
            style &= !(WS_HSCROLL.0 | ES_AUTOHSCROLL as u32);
            SetWindowLongPtrW(editor, GWL_STYLE, style as isize);
        }

        // Allow MB-sized documents in the editor. The default EDIT control
        // limit is 32KB of typed text — without this, typing/pasting a 5MB
        // file is silently truncated and the keyboard appears "stuck".
        send(editor, EM_SETLIMITTEXT, EDIT_TEXT_LIMIT, 0);
        // Same for the inline expand viewer (values can be multi-KB).
        send(expand_text, EM_SETLIMITTEXT, EDIT_TEXT_LIMIT, 0);

        apply_font_to_controls(&app);
        apply_dark_theme_to_views(&app);
        // Accept files dragged onto the window (Shell drag/drop).
        DragAcceptFiles(hwnd, BOOL(1));
        if let Some(path) = initial_file {
            match app.model.load_file(&path, &current) {
                Ok(_) => {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        set_window_text(hwnd, &format!("JSON Viewer — {}", name));
                    }
                }
                Err(e) => show_error(hwnd, &e),
            }
        }
        refresh_editor(&mut app);
        begin_tree_fill(&mut app, false);

        // Subclass search edit for Enter/Escape handling (the only child
        // subclass — editor/tree/grid stay native so SendMessage from the
        // main proc can never re-enter `app_of` and alias `&mut App`).
        let orig = SetWindowLongPtrW(search_edit, GWLP_WNDPROC, search_edit_proc as *const () as usize as isize);
        app.search_orig_proc = orig;
        SEARCH_ORIG_PROC.store(orig, std::sync::atomic::Ordering::Relaxed);

        let ptr = Box::into_raw(app);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
        APP_PTR = ptr as usize;

        // Show default tab from settings (model already set); ensure layout.
        if let Some(a) = app_of(hwnd) {
            layout(a);
        }
        true
    }
}

// ---------------- entry ----------------

pub fn run(
    settings: Arc<Mutex<Settings>>,
    store: Arc<Mutex<SettingsStore>>,
    initial_file: Option<std::path::PathBuf>,
) {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mut icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_TREEVIEW_CLASSES | ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES,
        };
        let _ = InitCommonControlsEx(&mut icc);

        let instance = HINSTANCE(GetModuleHandleW(None).unwrap_or_default().0);
        let class_name = wide("JSONViewerApp");
        // Dark window background (leaked for process lifetime, like the icon).
        let class_bg: &'static windows::Win32::Graphics::Gdi::HBRUSH =
            Box::leak(Box::new(CreateSolidBrush(COLORREF(DARK_CHROME_BG))));
        let wc = WNDCLASSW {
            style: windows::Win32::UI::WindowsAndMessaging::WNDCLASS_STYLES(0),
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hIcon: load_app_icon(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: *class_bg,
            lpszClassName: windows::core::PCWSTR::from_raw(class_name.as_ptr()),
            ..Default::default()
        };
        // class_name must stay alive through RegisterClassW + CreateWindowExW.
        let _ = RegisterClassW(&wc);

        let title = wide("JSON Viewer");
        let params = Box::new(CreateParams { settings, store, initial_file });
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows::core::PCWSTR::from_raw(class_name.as_ptr()),
            windows::core::PCWSTR::from_raw(title.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX,
            100,
            100,
            1120,
            740,
            None,
            None,
            instance,
            Some(Box::into_raw(params) as *const std::ffi::c_void),
        )
        .unwrap_or(HWND(std::ptr::null_mut()));
        if hwnd.is_invalid() {
            return;
        }
        // Dark title bar (Windows 10 1809+; ignored on older systems).
        {
            let dark = BOOL(1);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<BOOL>() as u32,
            );
        }
        let _ = ShowWindow(hwnd, SW_SHOW);
        let accel = build_accelerators();
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            // SAFETY: never hold `app_of()` (`&mut App`) across
            // TranslateAccelerator / TranslateMessage / DispatchMessage /
            // SendMessage — all of those synchronously re-enter `wnd_proc`
            // (or a subclass proc) which calls `app_of()` again. Holding the
            // outer `&mut` across them is aliased-mutable UB and was the
            // cause of the total UI breakage (blank editor, dead keys).
            // Pattern below: copy what we need in a short scope, drop the
            // borrow, then act.
            enum LoopAction {
                None,
                SkipAccelerator,
                TabIndent { editor: HWND, indent: i32 },
                FocusSearch,
                ShowShortcuts,
                /// Run search: 0 = fresh GO, 1 = next, -1 = previous
                /// (mac parity: Enter = next, Shift+Enter = previous).
                SearchAdvance { dir: i32 },
                /// Clear the search box + results (Escape in search box).
                ClearSearch,
            }
            let action: LoopAction = {
                let app = match app_of(hwnd) {
                    Some(a) => a,
                    None => {
                        // No app yet (startup): plain processing.
                        let use_accel = !accel.is_invalid()
                            && TranslateAcceleratorW(hwnd, accel, &mut msg) != 0;
                        if !use_accel {
                            let _ = TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                        continue;
                    }
                };
                let focus = GetFocus();
                let editor = app.ctrl.editor;
                let search_edit = app.ctrl.search_edit;
                let tree = app.ctrl.tree;
                let grid = app.ctrl.grid;
                let expand_text = app.ctrl.expand_text;
                let search_go = app.ctrl.search_go;
                let search_prev = app.ctrl.search_prev;
                let search_next = app.ctrl.search_next;
                let indent = app.settings.lock().unwrap().indent_spaces;
                let search_visible = !matches!(app.model.active_tab, AppTab::Text);
                let search_active = !app.model.search_query.is_empty()
                    || !app.model.search_results.is_empty();
                // Typing targets: plain '/' / '?' must reach them as text.
                let typing = focus == editor || focus == search_edit;
                // Borrow ends here (all copies).
                if msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN {
                    let vk = (msg.wParam.0 & 0xffff) as u32;
                    let ctrl_down = GetKeyState(0x11) < 0;
                    let alt_down = GetKeyState(0x12) < 0;
                    let shift_down = GetKeyState(0x10) < 0;
                    // Tab in the code editor inserts the configured indent
                    // (plain EDIT has no code-editor Tab support).
                    if vk == 0x09 && focus == editor && !ctrl_down && !alt_down {
                        LoopAction::TabIndent { editor, indent }
                    } else if vk == 0x1B && !ctrl_down && !alt_down
                        && (focus == search_edit || ((focus == tree || focus == grid || focus == expand_text) && search_visible))
                    {
                        // Escape clears the search from the box itself, and
                        // also when viewing results (tree/grid/expand focus).
                        // (The subclass handles the box too as a backup.)
                        LoopAction::ClearSearch
                    } else if vk == 0x0D && !ctrl_down && !alt_down && search_visible {
                        // Enter in the search box itself: next / Shift+Enter =
                        // previous (mac parity). Handled here rather than only
                        // in the subclass so it can't get lost in EDIT
                        // dispatch.
                        // Plain Enter also drives search from the GO/Previous/
                        // Next buttons (owner-drawn buttons don't activate on
                        // Enter by themselves) and from the tree/grid (which
                        // eats it) — otherwise Enter only worked after
                        // clicking GO.
                        if focus == search_edit {
                            LoopAction::SearchAdvance {
                                dir: if shift_down { -1 } else { 1 },
                            }
                        } else if focus == search_go {
                            LoopAction::SearchAdvance { dir: 0 }
                        } else if focus == search_prev {
                            LoopAction::SearchAdvance { dir: -1 }
                        } else if focus == search_next {
                            LoopAction::SearchAdvance { dir: 1 }
                        } else if (focus == tree || focus == grid || focus == expand_text) && search_active {
                            LoopAction::SearchAdvance {
                                dir: if shift_down { -1 } else { 1 },
                            }
                        } else {
                            LoopAction::None
                        }
                    } else if ctrl_down
                        && !alt_down
                        && (focus == editor || focus == search_edit || focus == expand_text)
                        && (vk == 0x43 || vk == 0x58)
                    {
                        // Let plain EDIT handle Ctrl+C / Ctrl+X natively
                        // (expand_text is read-only but selection copy/select
                        // must still come from the control itself).
                        LoopAction::SkipAccelerator
                    } else {
                        LoopAction::None
                    }
                } else if msg.message == WM_CHAR {
                    let ch = (msg.wParam.0 & 0xffff) as u32;
                    // Quick keys whenever NOT typing: tree/grid/expand/buttons.
                    // Must live in the message loop (not just wnd_proc WM_CHAR)
                    // because WM_CHAR addressed to a focused button/tree/grid
                    // never reaches the main window proc — without this, '/'
                    // silently did nothing after clicking any toolbar button.
                    // Works in every tab: '/' in Text tab switches to Split
                    // first (see FocusSearch), '?' always shows cheatsheet.
                    if (ch == '/' as u32 || ch == '?' as u32) && !typing
                    {
                        if ch == '/' as u32 {
                            LoopAction::FocusSearch
                        } else {
                            LoopAction::ShowShortcuts
                        }
                    } else {
                        LoopAction::None
                    }
                } else {
                    LoopAction::None
                }
            };
            match action {
                LoopAction::TabIndent { editor, indent } => {
                    let s = if indent < 0 {
                        "\t".to_string()
                    } else {
                        " ".repeat(indent as usize)
                    };
                    let w: Vec<u16> =
                        s.encode_utf16().chain(std::iter::once(0)).collect();
                    // EM_REPLACESEL = 0x00C2, wParam=1 (undoable).
                    send(editor, 0x00C2, 1, w.as_ptr() as isize);
                }
                LoopAction::FocusSearch => {
                    if let Some(app) = app_of(hwnd) {
                        if matches!(app.model.active_tab, AppTab::Text) {
                            set_tab(app, AppTab::Split);
                        } else {
                            let se = app.ctrl.search_edit;
                            let _ = SetFocus(se);
                        }
                        // set_tab already layouts; ensure focus when it didn't switch.
                        let _ = SetFocus(app.ctrl.search_edit);
                    }
                }
                LoopAction::ShowShortcuts => {
                    show_shortcuts(hwnd);
                }
                LoopAction::SearchAdvance { dir } => {
                    if let Some(app) = app_of(hwnd) {
                        do_search(app, dir);
                    }
                }
                LoopAction::ClearSearch => {
                    if let Some(app) = app_of(hwnd) {
                        clear_search_ui(app);
                    }
                }
                LoopAction::SkipAccelerator => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                LoopAction::None => {
                    let use_accel = !accel.is_invalid()
                        && TranslateAcceleratorW(hwnd, accel, &mut msg) != 0;
                    if !use_accel {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
        if !accel.is_invalid() {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyAcceleratorTable(accel);
        }
    }
}

#[allow(dead_code)]
fn _keep_alive() {
    // Referenced to keep refactors honest; intentionally empty.
    let _ = (WM_APP, WM_CLOSE, WM_DESTROY, TVS_SHOWSELALWAYS);
}
