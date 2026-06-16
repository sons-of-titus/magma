//! Clipboard integration using `arboard`.

use std::sync::Mutex;

/// Thread-safe wrapper around the system clipboard.
pub struct Clipboard {
    inner: Mutex<Option<arboard::Clipboard>>,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    pub fn new() -> Self {
        let clip = arboard::Clipboard::new().ok();
        Clipboard { inner: Mutex::new(clip) }
    }

    pub fn get_text(&self) -> Option<String> {
        let mut guard = self.inner.lock().ok()?;
        if let Some(ref mut clip) = *guard {
            clip.get_text().ok()
        } else {
            None
        }
    }

    pub fn set_text(&self, text: &str) {
        if let Ok(mut guard) = self.inner.lock()
            && let Some(ref mut clip) = *guard {
                let _ = clip.set_text(text);
            }
    }
}
