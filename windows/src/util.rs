//! Small Win32 string / window-text helpers shared by `app` and dialogs.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW, SetWindowTextW};

/// Encode as nul-terminated UTF-16 for Win32 calls.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn pcwstr(s: &[u16]) -> windows::core::PCWSTR {
    windows::core::PCWSTR::from_raw(s.as_ptr())
}

pub unsafe fn get_window_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let copied = GetWindowTextW(hwnd, &mut buf);
    if copied <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..copied as usize])
}

pub unsafe fn set_window_text(hwnd: HWND, text: &str) {
    let w = wide(text);
    let _ = SetWindowTextW(hwnd, pcwstr(&w));
}

/// Read potentially large EDIT control contents without truncation.
/// Normalizes Windows CRLF (and lone CR) to LF for the model.
pub unsafe fn get_edit_text(hwnd: HWND) -> String {
    // GetWindowTextLengthW can be unreliable for very large multi-line edits
    // in some configurations; fall back to WM_GETTEXTLENGTH.
    use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_GETTEXTLENGTH};
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    let len = SendMessageW(hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len + 1];
    let copied = GetWindowTextW(hwnd, &mut buf);
    if copied <= 0 {
        return String::new();
    }
    normalize_newlines(&String::from_utf16_lossy(&buf[..copied as usize]))
}

/// LF -> model form. No-op fast path for text without carriage returns.
pub fn normalize_newlines(s: &str) -> String {
    if !s.contains('\r') {
        return s.to_string();
    }
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Write model text (LF) into a multiline EDIT control. The native EDIT
/// control only breaks lines on CRLF — bare `\n` renders as one endless
/// single line (this is why startup/Format looked "not formatted").
pub unsafe fn set_editor_text(hwnd: HWND, text: &str) {
    let normalized = normalize_newlines(text);
    let with_crlf = normalized.replace('\n', "\r\n");
    let w = wide(&with_crlf);
    let _ = SetWindowTextW(hwnd, pcwstr(&w));
}
