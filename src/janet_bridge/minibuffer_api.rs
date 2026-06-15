//! Janet API — minibuffer: open, close, input, set-input, prompt, set-command-input.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::state::mode::{EditorMode, Minibuffer};

unsafe extern "C-unwind" fn c_minibuffer_open(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(prompt) = conv::get_str(argc, argv, 0) else { return conv::nil(); };
        let kind = conv::get_str(argc, argv, 1).unwrap_or_default();
        ed.editor_mode.minibuffer = Some(Minibuffer { prompt: prompt.clone(), input: String::new() });
        ed.plugin_state.insert("minibuffer.kind".to_string(), kind.clone());
        let mut payload = std::collections::HashMap::new();
        payload.insert("prompt".to_string(), prompt);
        payload.insert("kind".to_string(), kind);
        ed.events.emit("minibuffer-opened", payload);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_minibuffer_input(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        match &ed.editor_mode.minibuffer {
            Some(mb) => conv::string(&mb.input),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_minibuffer_set_input(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if let Some(text) = conv::get_str(argc, argv, 0) {
            if let Some(ref mut mb) = ed.editor_mode.minibuffer {
                mb.input = text;
                ed.events.emit("minibuffer-input-changed", std::collections::HashMap::new());
            }
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_minibuffer_close(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let (input, kind) = ed.editor_mode.minibuffer.take()
            .map(|mb| (mb.input, ed.plugin_state.get("minibuffer.kind").cloned().unwrap_or_default()))
            .unwrap_or_default();
        let mut payload = std::collections::HashMap::new();
        payload.insert("input".to_string(), input);
        payload.insert("kind".to_string(), kind);
        ed.events.emit("minibuffer-closed", payload);
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_minibuffer_prompt(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        match &ed.editor_mode.minibuffer {
            Some(mb) => conv::string(&mb.prompt),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_minibuffer_set_command_input(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(input) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        ed.editor_mode = EditorMode {
            name: "command".into(),
            accepts_text: false,
            minibuffer: Some(Minibuffer { prompt: ":".into(), input }),
        };
        conv::nil()
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"minibuffer/open".as_ptr() as *const _,
            cfun: Some(c_minibuffer_open as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Open the minibuffer with a prompt string".as_ptr() as *const _,
        },
        JanetReg {
            name: c"minibuffer/input".as_ptr() as *const _,
            cfun: Some(c_minibuffer_input as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the minibuffer input string, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"minibuffer/set-input".as_ptr() as *const _,
            cfun: Some(c_minibuffer_set_input as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Replace the minibuffer input string".as_ptr() as *const _,
        },
        JanetReg {
            name: c"minibuffer/close".as_ptr() as *const _,
            cfun: Some(c_minibuffer_close as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Close the minibuffer and emit minibuffer-closed".as_ptr() as *const _,
        },
        JanetReg {
            name: c"minibuffer/prompt".as_ptr() as *const _,
            cfun: Some(c_minibuffer_prompt as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return the minibuffer prompt string, or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"minibuffer/set-command-input".as_ptr() as *const _,
            cfun: Some(c_minibuffer_set_command_input as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Overwrite the command-mode minibuffer input string".as_ptr() as *const _,
        },
    ]
}
