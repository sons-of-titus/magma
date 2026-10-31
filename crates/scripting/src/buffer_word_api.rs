//! (buffer/word-under-cursor buf-id) → word or nil
//!
//! The word scan shared by goto-definition and multi-cursor: one rope pass,
//! no document copy.

use super::conv;
use super::with_editor;
use evil_janet::*;

unsafe extern "C-unwind" fn c_buffer_word_under_cursor(argc: i32, argv: *mut Janet) -> Janet {
    unsafe {
        with_editor(|ed| {
            let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
            let Some(view) = ed.view_of_buffer(key) else {
                return conv::nil();
            };
            let pos = view.cursor_offset();
            let buf = view.buffer.lock().unwrap_or_else(|e| e.into_inner());
            let is_word = |c: char| c.is_alphanumeric() || c == '_';
            let start = buf
                .rfind_char_before(pos, |c| !is_word(c))
                .map(|i| i + 1)
                .unwrap_or(0);
            let end = buf
                .find_char_from(start, |c| !is_word(c))
                .unwrap_or(buf.len());
            if start >= end {
                return conv::nil();
            }
            conv::string(&buf.slice(start, end))
        })
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![JanetReg {
        name: c"buffer/word-under-cursor".as_ptr() as *const _,
        cfun: Some(c_buffer_word_under_cursor),
        documentation:
            c"(buffer/word-under-cursor buf-id) → word or nil — the word at the cursor position"
                .as_ptr() as *const _,
    }]
}
