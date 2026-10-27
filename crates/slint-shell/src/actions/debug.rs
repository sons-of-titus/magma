//! Debug tool window wiring: execution commands, frame selection, variables
//! expansion, watches, and breakpoint management.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use slint::ComponentHandle;

use crate::{AppState, AppWindow, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_frame_selected(move |slot| {
            let mut s = state.borrow_mut();
            let Some(id) = s.debug_ref_slots.get(slot.max(0) as usize).copied() else {
                return;
            };
            exec(&mut s, "dbg-frame", ("frame", ArgValue::Integer(id)));
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_toggle_expand(move |var_ref| {
            let mut s = state.borrow_mut();
            // Collapsing is pure UI state; expanding asks the adapter for
            // the children it has not fetched yet.
            let var_ref = var_ref as i64;
            if !s.debug_expanded.remove(&var_ref) {
                s.debug_expanded.insert(var_ref);
                exec(
                    &mut s,
                    "dbg-expand",
                    ("reference", ArgValue::Integer(var_ref)),
                );
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_watch_add(move || {
            let Some(app) = weak.upgrade() else { return };
            let expr = app.get_debug_watch_draft().trim().to_string();
            if expr.is_empty() {
                return;
            }
            let mut s = state.borrow_mut();
            exec(&mut s, "dbg-watch-add", ("expr", ArgValue::String(expr)));
            app.set_debug_watch_draft("".into());
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_watch_remove(move |expr| {
            let mut s = state.borrow_mut();
            exec(
                &mut s,
                "dbg-watch-remove",
                ("expr", ArgValue::String(expr.to_string())),
            );
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_bp_toggle(move |file, line, enabled| {
            let mut s = state.borrow_mut();
            exec_bp(
                &mut s,
                "dbg-bp-enable",
                &file,
                line,
                Some(ArgValue::Boolean(enabled)),
            );
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_bp_remove(move |file, line| {
            let mut s = state.borrow_mut();
            exec_bp(&mut s, "dbg-bp-remove", &file, line, None);
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_bp_open(move |file, line| {
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(file.to_string()));
            if execute_command(&mut s.editor, "open-file", &args).is_ok() {
                let mut args = HashMap::new();
                args.insert("line".to_string(), ArgValue::Integer(line as i64));
                if let Err(e) = execute_command(&mut s.editor, "goto-line", &args) {
                    s.message = e;
                }
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_sidebar_resized(move |delta| {
            let mut s = state.borrow_mut();
            let rail = s.rail.clamp(0, 2) as usize;
            let current = s.sidebar_widths[rail];
            s.sidebar_widths[rail] = (current + delta).clamp(240.0, 560.0);
            refresh(&weak, &mut s);
        });
    }
}

fn exec(s: &mut AppState, command: &str, (name, value): (&str, ArgValue)) {
    let mut args = HashMap::new();
    args.insert(name.to_string(), value);
    match execute_command(&mut s.editor, command, &args) {
        Ok(()) => s.message.clear(),
        Err(e) => s.message = e,
    }
}

fn exec_bp(s: &mut AppState, command: &str, file: &str, line: i32, extra: Option<ArgValue>) {
    let mut args = HashMap::new();
    args.insert("path".to_string(), ArgValue::Path(file.to_string()));
    args.insert("line".to_string(), ArgValue::Integer(line as i64));
    if let Some(v) = extra {
        args.insert("enabled".to_string(), v);
    }
    match execute_command(&mut s.editor, command, &args) {
        Ok(()) => s.message.clear(),
        Err(e) => s.message = e,
    }
}
