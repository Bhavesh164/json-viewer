//! Clipboard helpers for Linux (Wayland + X11).
//! Primary path is `arboard` (works on both Wayland and X11). The egui
//! context also mirrors copies via `ctx.copy_text_to_clipboard`, so callers
//! should do both: egui for the current frame + arboard for reliability.

pub fn set_text(text: &str) -> bool {
    match arboard::Clipboard::new() {
        Ok(mut cb) => cb.set_text(text.to_string()).is_ok(),
        Err(_) => false,
    }
}

pub fn get_text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}
