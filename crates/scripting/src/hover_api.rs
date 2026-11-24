//! (hover/attach word kind text) → boolean
//!
//! Language plugins augment the pointer-dwell hover card: the shell emits
//! a `hover-card` event when a card is created, and a plugin appends
//! tagged lines for the matching word.

use super::conv;
use super::with_editor;
use evil_janet::*;

unsafe extern "C-unwind" fn c_hover_attach(argc: i32, argv: *mut Janet) -> Janet {
    unsafe {
        with_editor(|ed| {
            let Some(word) = conv::get_str(argc, argv, 0) else {
                return conv::boolean(false);
            };
            let kind = conv::get_str(argc, argv, 1).unwrap_or_else(|| "text".to_string());
            let Some(text) = conv::get_str(argc, argv, 2) else {
                return conv::boolean(false);
            };
            let Some(card) = ed.hover.as_mut() else {
                return conv::boolean(false);
            };
            // The pointer may have moved on since the event was emitted —
            // only augment the card that is still showing this word.
            if card.word != word {
                return conv::boolean(false);
            }
            card.lines.push(kernel::state::HoverLine { kind, text });
            conv::boolean(true)
        })
    }
}

pub fn register() -> Vec<JanetReg> {
    vec![JanetReg {
        name: c"hover/attach".as_ptr() as *const _,
        cfun: Some(c_hover_attach),
        documentation: c"(hover/attach word kind text) → boolean — append a styled line to the hover card showing word; false when no matching card is on screen"
            .as_ptr() as *const _,
    }]
}
