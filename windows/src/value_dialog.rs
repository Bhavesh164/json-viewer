//! "Expand full text" dialog — the Windows equivalent of the macOS
//! expandable big-text view. Shows the complete (possibly multi-KB) string
//! value wrapped in a read-only editor with a character count and Copy.
//!
//! Deliberately uses the plain system theme: it is a transient utility
//! dialog, like the native file pickers.

use crate::clipboard;
use crate::util::{get_window_text, set_window_text, wide};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, RegisterClassW,
    SetWindowLongPtrW, ShowWindow, BS_PUSHBUTTON, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE,
    ES_READONLY, ES_WANTRETURN, GWLP_USERDATA, HMENU, SW_SHOW, WNDCLASSW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WS_BORDER, WS_CAPTION, WS_CHILD, WS_SYSMENU, WS_VISIBLE, WS_VSCROLL,
};

const IDC_TEXT: u32 = 4001;
const IDC_COUNT: u32 = 4002;
const IDC_COPY: u32 = 4003;
const IDC_CLOSE: u32 = 4004;

struct DialogState {
    full_text: String,
    text_hwnd: HWND,
}

fn make_child(
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
    unsafe {
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
}

extern "system" fn dlg_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        const WM_COMMAND: u32 = 0x0111;
        const WM_DESTROY: u32 = 0x0002;
        const WM_SIZE: u32 = 0x0005;
        if msg == WM_COMMAND {
            let id = (wparam.0 & 0xffff) as u32;
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
            if id == IDC_CLOSE {
                let _ = DestroyWindow(hwnd);
                return LRESULT(0);
            } else if id == IDC_COPY && !ptr.is_null() {
                let text = get_window_text((*ptr).text_hwnd);
                if clipboard::set_text(&text) {
                    let note = wide("Copied to clipboard!");
                    let _ = windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                        hwnd,
                        windows::core::PCWSTR::from_raw(note.as_ptr()),
                        windows::core::PCWSTR::from_raw(wide("JSON Viewer").as_ptr()),
                        windows::Win32::UI::WindowsAndMessaging::MB_OK,
                    );
                }
                return LRESULT(0);
            }
        } else if msg == WM_SIZE {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
            if !ptr.is_null() {
                let w = (lparam.0 & 0xffff) as i32;
                let h = ((lparam.0 >> 16) & 0xffff) as i32;
                let _ = windows::Win32::UI::WindowsAndMessaging::SetWindowPos(
                    (*ptr).text_hwnd,
                    HWND(std::ptr::null_mut()),
                    12,
                    40,
                    (w - 24).max(50),
                    (h - 130).max(50),
                    windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
                );
            }
            return LRESULT(0);
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

/// Show the full text of a (possibly huge) string value, mac-expand style.
pub fn open_value_dialog(key: &str, value: &str) {
    unsafe {
        let instance = HINSTANCE(
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .unwrap_or_default()
                .0,
        );
        static mut REGISTERED: bool = false;
        if !REGISTERED {
            let cls = wide("JSONViewerValue");
            let leaked: &'static [u16] = Box::leak(cls.into_boxed_slice());
            let wc = WNDCLASSW {
                style: windows::Win32::UI::WindowsAndMessaging::WNDCLASS_STYLES(0),
                lpfnWndProc: Some(dlg_proc),
                hInstance: instance,
                hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                    (windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 as isize + 1)
                        as *mut std::ffi::c_void,
                ),
                lpszClassName: windows::core::PCWSTR::from_raw(leaked.as_ptr()),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);
            REGISTERED = true;
        }

        let title_text = if key.chars().count() > 60 {
            format!("Value — {}…", key.chars().take(60).collect::<String>())
        } else {
            format!("Value — {}", key)
        };
        let cls = wide("JSONViewerValue");
        let title = wide(&title_text);
        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WS_EX_DLGMODALFRAME,
            windows::core::PCWSTR::from_raw(cls.as_ptr()),
            windows::core::PCWSTR::from_raw(title.as_ptr()),
            WINDOW_STYLE(WS_VISIBLE.0 | WS_BORDER.0 | WS_CAPTION.0 | WS_SYSMENU.0),
            160,
            160,
            640,
            480,
            None,
            None,
            instance,
            None,
        )
        .unwrap_or(HWND(std::ptr::null_mut()));
        if hwnd.is_invalid() {
            return;
        }

        let chars = value.encode_utf16().count();
        let _ = make_child(
            hwnd,
            "STATIC",
            &format!("{} characters", chars),
            IDC_COUNT,
            12,
            12,
            400,
            20,
            WINDOW_STYLE(0),
            instance,
        );
        let text_hwnd = make_child(
            hwnd,
            "EDIT",
            "",
            IDC_TEXT,
            12,
            40,
            600,
            350,
            WINDOW_STYLE(
                WS_BORDER.0
                    | WS_VSCROLL.0
                    | ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_WANTRETURN as u32
                    | ES_READONLY as u32
                    | ES_AUTOHSCROLL as u32,
            ),
            instance,
        );
        // Word-wrap ON (no horizontal scroll style adjustments needed beyond
        // omitting WS_HSCROLL): long lines wrap instead of scrolling.
        set_window_text(text_hwnd, value);

        let _ = make_child(hwnd, "BUTTON", "Copy", IDC_COPY, 400, 402, 100, 28,
            WINDOW_STYLE(BS_PUSHBUTTON as u32), instance);
        let _ = make_child(hwnd, "BUTTON", "Close", IDC_CLOSE, 510, 402, 100, 28,
            WINDOW_STYLE(BS_PUSHBUTTON as u32), instance);

        let state = Box::new(DialogState { full_text: value.to_string(), text_hwnd });
        let _ = &state.full_text;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        let _ = ShowWindow(hwnd, SW_SHOW);
    }
}
