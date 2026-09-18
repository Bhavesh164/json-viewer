//! Settings dialog — Win32 port of `SettingsView.swift`.
//! Follows the same manual-control pattern as `mouseless/windows/src/preferences.rs`.

use crate::settings::Settings;
use crate::util::{get_window_text, set_window_text, wide};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::SystemServices::SS_LEFT;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, RegisterClassW,
    SendMessageW, SetWindowLongPtrW, ShowWindow, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX,
    BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST, ES_AUTOHSCROLL,
    ES_NUMBER, GWLP_USERDATA, HMENU, SW_SHOW, WNDCLASSW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_BORDER,
    WS_CAPTION, WS_CHILD, WS_SYSMENU, WS_VISIBLE, WS_VSCROLL,
};

pub const IDC_INDENT: u32 = 3001;
pub const IDC_SORT: u32 = 3002;
pub const IDC_ESCAPE: u32 = 3003;
pub const IDC_UNWRAP: u32 = 3004;
pub const IDC_WRAP: u32 = 3005;
pub const IDC_DEFTAB: u32 = 3006;
pub const IDC_FONTSIZE: u32 = 3007;
pub const IDC_RESET: u32 = 3008;
pub const IDC_DONE: u32 = 3009;

pub struct SettingsContext {
    pub settings: Arc<Mutex<Settings>>,
    pub on_apply: Arc<dyn Fn() + Send + Sync>,
}

struct Controls {
    indent: HWND,
    sort: HWND,
    escape: HWND,
    unwrap: HWND,
    wrap: HWND,
    deftab: HWND,
    fontsize: HWND,
}

struct DialogState {
    ctx: SettingsContext,
    ctrl: Controls,
}

fn combo_style(extra: u32) -> WINDOW_STYLE {
    WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | extra)
}

unsafe fn create_control(
    parent: HWND,
    class_name: &str,
    text: &str,
    id: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: WINDOW_STYLE,
    instance: HINSTANCE,
) -> HWND {
    let cls = wide(class_name);
    let txt = wide(text);
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        windows::core::PCWSTR::from_raw(cls.as_ptr()),
        windows::core::PCWSTR::from_raw(txt.as_ptr()),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style.0),
        x,
        y,
        w,
        h,
        parent,
        HMENU(id as usize as *mut std::ffi::c_void),
        instance,
        None,
    )
    .unwrap_or(HWND(std::ptr::null_mut()))
}

unsafe fn is_checked(hwnd: HWND) -> bool {
    (SendMessageW(hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 & 1) != 0
}

unsafe fn set_checked(hwnd: HWND, checked: bool) {
    let _ = SendMessageW(hwnd, BM_SETCHECK, WPARAM(if checked { 1 } else { 0 }), LPARAM(0));
}

unsafe fn combo_add(hwnd: HWND, text: &str) {
    let w = wide(text);
    let _ = SendMessageW(
        hwnd,
        CB_ADDSTRING,
        WPARAM(0),
        LPARAM(w.as_ptr() as isize),
    );
}

unsafe fn combo_get(hwnd: HWND) -> i32 {
    SendMessageW(hwnd, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32
}

unsafe fn combo_set(hwnd: HWND, idx: usize) {
    let _ = SendMessageW(hwnd, CB_SETCURSEL, WPARAM(idx), LPARAM(0));
}

extern "system" fn dlg_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        const WM_COMMAND: u32 = 0x0111;
        const WM_DESTROY: u32 = 0x0002;
        if msg == WM_COMMAND {
            let id = (wparam.0 & 0xffff) as u32;
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
            if id == IDC_DONE {
                if !ptr.is_null() {
                    apply(&*ptr);
                }
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            } else if id == IDC_RESET {
                if !ptr.is_null() {
                    let state = &*ptr;
                    {
                        let mut s = state.ctx.settings.lock().unwrap();
                        s.reset_to_defaults();
                    }
                    refresh_controls(state);
                    (state.ctx.on_apply)();
                }
                return LRESULT(0);
            }
        } else if msg == WM_DESTROY {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
            if !ptr.is_null() {
                let _ = Box::from_raw(ptr);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            return LRESULT(0);
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

unsafe fn apply(state: &DialogState) {
    let mut s = state.ctx.settings.lock().unwrap();
    s.indent_spaces = match combo_get(state.ctrl.indent) {
        1 => 4,
        2 => -1,
        _ => 2,
    };
    s.sort_keys_alphabetically = is_checked(state.ctrl.sort);
    s.escape_slashes_in_stringify = is_checked(state.ctrl.escape);
    s.auto_unwrap_stringified = is_checked(state.ctrl.unwrap);
    s.wrap_lines = is_checked(state.ctrl.wrap);
    s.default_tab = match combo_get(state.ctrl.deftab) {
        1 => "Viewer".to_string(),
        2 => "Split".to_string(),
        _ => "Text".to_string(),
    };
    if let Ok(v) = get_window_text(state.ctrl.fontsize).parse::<f64>() {
        s.font_size = v.clamp(9.0, 24.0);
    }
    drop(s);
    (state.ctx.on_apply)();
}

unsafe fn refresh_controls(state: &DialogState) {
    let s = state.ctx.settings.lock().unwrap().clone();
    combo_set(state.ctrl.indent, match s.indent_spaces {
        4 => 1,
        -1 => 2,
        _ => 0,
    });
    set_checked(state.ctrl.sort, s.sort_keys_alphabetically);
    set_checked(state.ctrl.escape, s.escape_slashes_in_stringify);
    set_checked(state.ctrl.unwrap, s.auto_unwrap_stringified);
    set_checked(state.ctrl.wrap, s.wrap_lines);
    combo_set(state.ctrl.deftab, match s.default_tab.as_str() {
        "Viewer" => 1,
        "Split" => 2,
        _ => 0,
    });
    set_window_text(state.ctrl.fontsize, &format!("{}", s.font_size as i32));
}

pub fn open_settings(ctx: SettingsContext) {
    unsafe {
        let instance = HINSTANCE(
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .0,
        );
        static mut REGISTERED: bool = false;
        if !REGISTERED {
            let cls = wide("JSONViewerSettings");
            let wc = WNDCLASSW {
                style: windows::Win32::UI::WindowsAndMessaging::WNDCLASS_STYLES(0),
                lpfnWndProc: Some(dlg_proc),
                hInstance: instance,
                lpszClassName: windows::core::PCWSTR::from_raw(cls.as_ptr()),
                ..Default::default()
            };
            // NOTE: cls must outlive RegisterClassW — it copies the name, so
            // leaking here is intentional and matches the mouseless pattern.
            let leaked: &'static [u16] = Box::leak(cls.into_boxed_slice());
            let mut wc2 = wc;
            wc2.lpszClassName = windows::core::PCWSTR::from_raw(leaked.as_ptr());
            let _ = RegisterClassW(&wc2);
            REGISTERED = true;
        }

        let s = ctx.settings.lock().unwrap().clone();
        let cls = wide("JSONViewerSettings");
        let title = wide("JSON Viewer Settings");
        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WS_EX_DLGMODALFRAME,
            windows::core::PCWSTR::from_raw(cls.as_ptr()),
            windows::core::PCWSTR::from_raw(title.as_ptr()),
            WINDOW_STYLE(WS_VISIBLE.0 | WS_BORDER.0 | WS_CAPTION.0 | WS_SYSMENU.0),
            140,
            140,
            440,
            400,
            None,
            None,
            instance,
            None,
        )
        .unwrap_or(HWND(std::ptr::null_mut()));
        if hwnd.is_invalid() {
            return;
        }

        let label = |text: &str, x: i32, y: i32, w: i32| {
            create_control(hwnd, "STATIC", text, 0, x, y, w, 22, WINDOW_STYLE(SS_LEFT.0), instance)
        };

        let _ = label("Indentation", 16, 16, 150);
        let indent = create_control(hwnd, "COMBOBOX", "", IDC_INDENT, 200, 14, 200, 120,
            combo_style(WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32), instance);
        combo_add(indent, "2 Spaces (Default)");
        combo_add(indent, "4 Spaces");
        combo_add(indent, "Tabs");

        let sort = create_control(hwnd, "BUTTON", "Sort object keys alphabetically", IDC_SORT,
            16, 48, 380, 22, WINDOW_STYLE(0x00000003 /*BS_AUTOCHECKBOX*/ as u32), instance);
        let escape = create_control(hwnd, "BUTTON", "Escape forward slashes (\\/) when stringifying", IDC_ESCAPE,
            16, 76, 380, 22, WINDOW_STYLE(BS_AUTOCHECKBOX as u32), instance);
        let unwrap = create_control(hwnd, "BUTTON", "Auto-unwrap stringified JSON", IDC_UNWRAP,
            16, 104, 380, 22, WINDOW_STYLE(BS_AUTOCHECKBOX as u32), instance);
        let wrap = create_control(hwnd, "BUTTON", "Wrap long lines in editor", IDC_WRAP,
            16, 132, 380, 22, WINDOW_STYLE(BS_AUTOCHECKBOX as u32), instance);

        let _ = label("Default tab on launch", 16, 164, 170);
        let deftab = create_control(hwnd, "COMBOBOX", "", IDC_DEFTAB, 200, 162, 200, 120,
            combo_style(WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32), instance);
        combo_add(deftab, "Text Editor");
        combo_add(deftab, "Tree Viewer");
        combo_add(deftab, "Split View");

        let _ = label("Font size (9-24 pt)", 16, 196, 170);
        let fontsize = create_control(hwnd, "EDIT", "", IDC_FONTSIZE, 200, 194, 80, 22,
            WINDOW_STYLE(WS_BORDER.0 | ES_AUTOHSCROLL as u32 | ES_NUMBER as u32), instance);

        let _ = create_control(hwnd, "BUTTON", "Reset to Defaults", IDC_RESET, 40, 240, 150, 30,
            WINDOW_STYLE(BS_PUSHBUTTON as u32), instance);
        let _ = create_control(hwnd, "BUTTON", "Done", IDC_DONE, 240, 240, 120, 30,
            WINDOW_STYLE(BS_PUSHBUTTON as u32), instance);

        let _ = sort;
        combo_set(indent, match s.indent_spaces { 4 => 1, -1 => 2, _ => 0 });
        set_checked(sort, s.sort_keys_alphabetically);
        set_checked(escape, s.escape_slashes_in_stringify);
        set_checked(unwrap, s.auto_unwrap_stringified);
        set_checked(wrap, s.wrap_lines);
        combo_set(deftab, match s.default_tab.as_str() { "Viewer" => 1, "Split" => 2, _ => 0 });
        set_window_text(fontsize, &format!("{}", s.font_size as i32));

        let ctrl = Controls { indent, sort, escape, unwrap, wrap, deftab, fontsize };
        let state = Box::new(DialogState { ctx, ctrl });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
}
