//! Clipboard helpers (Unicode text) via Win32 DataExchange APIs.

use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

pub fn set_text(text: &str) -> bool {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = (wide.len() * 2) as usize;
        let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, bytes) else {
            return false;
        };
        let ptr = GlobalLock(hmem);
        if ptr.is_null() {
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, bytes);
        let _ = GlobalUnlock(hmem);
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let ok = SetClipboardData(CF_UNICODETEXT.0 as u32, HANDLE(hmem.0 as isize as *mut std::ffi::c_void)).is_ok();
        let _ = CloseClipboard();
        ok
    }
}

pub fn get_text() -> Option<String> {
    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
        let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0 as isize as *mut std::ffi::c_void));
        if ptr.is_null() {
            let _ = CloseClipboard();
            return None;
        }
        // Measure nul-terminated UTF-16 length.
        let mut len = 0usize;
        loop {
            let unit = *(ptr as *const u16).add(len);
            if unit == 0 {
                break;
            }
            len += 1;
            if len > 16_000_000 {
                break;
            }
        }
        let slice = std::slice::from_raw_parts(ptr as *const u16, len);
        let text = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0 as isize as *mut std::ffi::c_void));
        let _ = CloseClipboard();
        Some(text)
    }
}
