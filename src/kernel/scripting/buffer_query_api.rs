//! Janet API — buffer queries: current, list, create, name, path, mode, line info, modified, diagnostics.

use evil_janet::*;
use super::conv;
use super::with_editor;
use crate::kernel::event::payload::*;
use crate::kernel::event::keys;

pub(super) fn emit_buffer_changed(ed: &mut crate::kernel::state::Editor, key: usize) {
    ed.events.emit_typed(keys::events::BUFFER_CHANGED, BufferChangedPayload {
        buffer_id: key.to_string(),
    });
}

unsafe extern "C-unwind" fn c_buffer_current(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let slab = ed.windows.focused_window()
            .and_then(|wid| ed.windows.buffer(wid));
        debug!("BC: buffer/current -> {:?}", slab);
        match slab {
            Some(k) => conv::integer(k as i32),
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let arr = janet_wrap_array(janet_array(ed.buffers.len() as i32));
        for (key, buf) in ed.buffers.iter() {
            let tbl = janet_wrap_table(janet_table(0));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("slab"), conv::integer(key as i32));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("name"), conv::string(&buf.name));
            janet_table_put(janet_unwrap_table(tbl), conv::keyword("len"), conv::integer(buf.len() as i32));
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

unsafe extern "C-unwind" fn c_buffer_create(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = conv::get_str(argc, argv, 0).unwrap_or_default();
        let content = conv::get_str(argc, argv, 1).unwrap_or_default();
        let buf_id = ed.allocate_buffer_id();
        let buf = if content.is_empty() {
            crate::kernel::text_engine::Buffer::new(crate::kernel::state::id::BufferId(buf_id), &name)
        } else {
            crate::kernel::text_engine::Buffer::from_string(crate::kernel::state::id::BufferId(buf_id), &name, &content)
        };
        let entry = ed.buffers.vacant_entry();
        let key = entry.key();
        entry.insert(buf);
        if !content.is_empty() {
            emit_buffer_changed(&mut *ed, key);
        }
        conv::integer(key as i32)
    })
}

unsafe extern "C-unwind" fn c_buffer_name(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let name = ed.buffers.get(key).map(|b| b.name.clone()).unwrap_or_default();
        conv::string(&name)
    })
}

unsafe extern "C-unwind" fn c_buffer_path(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        match ed.buffers.get(key) {
            Some(buf) => match &buf.path {
                Some(p) => conv::string(p),
                None => conv::nil(),
            },
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_set_path(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let Some(path) = conv::get_str(argc, argv, 1) else {
            return conv::nil();
        };
        if let Some(buf) = ed.buffers.get_mut(key) {
            buf.path = Some(path);
        }
        conv::nil()
    })
}

unsafe extern "C-unwind" fn c_buffer_major_mode(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        match ed.buffers.get(key) {
            Some(buf) => conv::string(buf.major_mode.name()),
            None => conv::string("fundamental"),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_line_count(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        match ed.buffers.get(key) {
            Some(buf) => conv::integer(buf.line_count() as i32),
            None => conv::integer(0),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_line_start_offset(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let line = conv::get_int(argc, argv, 1).unwrap_or(0) as usize;
        match ed.buffers.get(key) {
            Some(buf) => match buf.line_start_offset(line) {
                Some(offset) => conv::integer(offset as i32),
                None => conv::nil(),
            },
            None => conv::nil(),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_line_number(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        match ed.buffers.get(key) {
            Some(buf) => {
                let cursor = buf.cursor();
                let text = buf.slice(0, cursor);
                let line = text.chars().filter(|&c| c == '\n').count();
                conv::integer(line as i32)
            }
            None => conv::integer(0),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_modified(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let key = unsafe { conv::get_int(argc, argv, 0) }.unwrap_or(-1) as usize;
        match ed.buffers.get(key) {
            Some(buf) => conv::boolean(buf.modified()),
            None => conv::boolean(false),
        }
    })
}

unsafe extern "C-unwind" fn c_buffer_diagnostics(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let key = conv::get_int(argc, argv, 0).unwrap_or(-1) as usize;
        let diags = match ed.buffers.get(key) {
            Some(buf) => buf.get_diagnostics().to_vec(),
            None => Vec::new(),
        };
        let arr_ptr = janet_array(diags.len() as i32);
        for d in diags {
            janet_array_push(arr_ptr, conv::string(&d));
        }
        janet_wrap_array(arr_ptr)
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"buffer/current".as_ptr() as *const _,
            cfun: Some(c_buffer_current as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get current buffer slab key".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/list".as_ptr() as *const _,
            cfun: Some(c_buffer_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"List all buffers".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/create".as_ptr() as *const _,
            cfun: Some(c_buffer_create as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Create a new buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/name".as_ptr() as *const _,
            cfun: Some(c_buffer_name as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get buffer name".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/path".as_ptr() as *const _,
            cfun: Some(c_buffer_path as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get buffer file path or nil".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/set-path".as_ptr() as *const _,
            cfun: Some(c_buffer_set_path as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Set the file path for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/major-mode".as_ptr() as *const _,
            cfun: Some(c_buffer_major_mode as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get the major mode name for a buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/line-count".as_ptr() as *const _,
            cfun: Some(c_buffer_line_count as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get number of lines in buffer".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/line-start-offset".as_ptr() as *const _,
            cfun: Some(c_buffer_line_start_offset as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get byte offset of a line start".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/line-number".as_ptr() as *const _,
            cfun: Some(c_buffer_line_number as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get current line number (0-based)".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/modified?".as_ptr() as *const _,
            cfun: Some(c_buffer_modified as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return true if the buffer has unsaved changes".as_ptr() as *const _,
        },
        JanetReg {
            name: c"buffer/diagnostics".as_ptr() as *const _,
            cfun: Some(c_buffer_diagnostics as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Get LSP diagnostics for a buffer".as_ptr() as *const _,
        },
    ]
}
