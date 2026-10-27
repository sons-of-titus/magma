//! Callback wiring: Slint signals → commands and state changes.
//!
//! Every component callback lands here.  Actions go through the command
//! registry whenever one exists, so the Slint shell behaves exactly like the
//! keymap, the menu, and the egui chrome.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use magma::kernel::command::args::ArgValue;
use magma::kernel::command::execute_command;
use magma::kernel::render::tabs;
use slint::ComponentHandle;

use crate::{input, logical_size, pane, projection, refresh, AppState, AppWindow};

pub(crate) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_key(move |text, ctrl, shift, alt, meta| {
            let Some(key) = input::translate_key(&text, ctrl, shift, alt, meta) else { return };
            let mut s = state.borrow_mut();
            magma::kernel::input::dispatch_key(&mut s.editor, &key);
            s.editor.dispatch_events_scoped();
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_tab_clicked(move |index| {
            let mut s = state.borrow_mut();
            let tabs = tabs::collect_tabs(&s.editor);
            if let Some(tab) = tabs.get(index.max(0) as usize) {
                s.editor.view_tree.focus(tab.window_id);
                s.editor.frame_dirty = true;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_tab_closed(move |index| {
            let mut s = state.borrow_mut();
            let tabs = tabs::collect_tabs(&s.editor);
            if let Some(tab) = tabs.get(index.max(0) as usize) {
                tabs::close_tab(&mut s.editor, tab.window_id);
                s.editor.frame_dirty = true;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_tree_clicked(move |path| {
            let path = path.to_string();
            let mut s = state.borrow_mut();
            if s.dirs.contains(&path) {
                if !s.expanded.remove(&path) {
                    s.expanded.insert(path);
                }
            } else {
                let mut args = HashMap::new();
                args.insert("path".to_string(), ArgValue::Path(path));
                match execute_command(&mut s.editor, "open-file", &args) {
                    Ok(()) => s.message.clear(),
                    Err(e) => s.message = e,
                }
                s.editor.frame_dirty = true;
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_rail_selected(move |index| {
            let mut s = state.borrow_mut();
            s.rail = index;
            if index == 1 {
                let _ = execute_command(&mut s.editor, "vc-refresh", &HashMap::new());
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_menu_opened(move |title, x| {
            let mut s = state.borrow_mut();
            let width = weak
                .upgrade()
                .map(|a| {
                    let (w, _) = logical_size(&a);
                    w
                })
                .unwrap_or(1200.0);
            s.menu_title = Some(projection::menu_title(&title));
            s.menu_x = x.min(width - 212.0).max(4.0);
            s.menu_visible = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_command(move |command| {
            let mut s = state.borrow_mut();
            let command = command.to_string();
            match command.as_str() {
                "open-project-dialog" => {
                    if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                        let mut args = HashMap::new();
                        args.insert(
                            "path".to_string(),
                            ArgValue::Path(dir.to_string_lossy().into_owned()),
                        );
                        match execute_command(&mut s.editor, "open-project", &args) {
                            Ok(()) => s.message.clear(),
                            Err(e) => s.message = e,
                        }
                        s.tree_epoch = u64::MAX;
                    }
                }
                "toggle-theme" => {
                    let next = match s.theme_name.as_str() {
                        "intellij-dark" => "intellij-light",
                        "intellij-light" => "mocha",
                        "mocha" => "latte",
                        _ => "intellij-dark",
                    };
                    s.editor.options.insert("gui-theme".into(), next.into());
                    s.theme_name.clear();
                }
                "rail:0" | "rail:1" | "rail:2" => {
                    s.rail = command.trim_start_matches("rail:").parse().unwrap_or(0);
                    if s.rail == 1 {
                        let _ = execute_command(&mut s.editor, "vc-refresh", &HashMap::new());
                    }
                }
                "dialog:settings" => {
                    s.settings_visible = true;
                    if let Some(a) = weak.upgrade() { a.set_settings_visible(true); }
                }
                c if c.starts_with("settings:theme:") => {
                    let name = c.trim_start_matches("settings:theme:");
                    s.editor.options.insert("gui-theme".into(), name.into());
                    s.theme_name.clear();
                }
                c if c.starts_with("settings:font:") => {
                    let (ui, small) = match c.trim_start_matches("settings:font:") {
                        "small" => (11.0f32, 10.0f32),
                        "large" => (16.0f32, 14.0f32),
                        _ => (13.0f32, 11.0f32),
                    };
                    if let Some(a) = weak.upgrade() {
                        let g = a.global::<crate::Theme>();
                        g.set_font_ui(ui);
                        g.set_font_small(small);
                    }
                }
                "toggle-minimap" => {
                    let cur = s.editor.options.get("minimap-enabled").map(|v| v.as_str()) == Some("true");
                    s.editor.options.insert("minimap-enabled".into(), if cur { "false".into() } else { "true".into() });
                }
                "toggle-mouse" => {
                    let cur = s.editor.options.get("mouse-support").map(|v| v.as_str()) == Some("true");
                    s.editor.options.insert("mouse-support".into(), if cur { "false".into() } else { "true".into() });
                }
                "toggle-line-numbers" => {
                    let cur = s.editor.options.get("number").map(|v| v.as_str()) == Some("true");
                    s.editor.options.insert("number".into(), if cur { "false".into() } else { "true".into() });
                }
                "toggle-wrap" => {
                    let cur = s.editor.options.get("wrap").map(|v| v.as_str()) == Some("true");
                    s.editor.options.insert("wrap".into(), if cur { "false".into() } else { "true".into() });
                }
                "toggle-auto-save" => {
                    let cur = s.editor.options.get("auto-save").map(|v| v.as_str()) == Some("true");
                    s.editor.options.insert("auto-save".into(), if cur { "false".into() } else { "true".into() });
                }
                "dialog:find" => {
                    s.dialog_title = "Find in Buffers".into();
                    s.dialog_input.clear();
                    s.dialog_visible = true;
                    if let Some(a) = weak.upgrade() { a.set_dialog_input("".into()); }
                }
                "dialog:goto-file" => {
                    s.dialog_title = "Go to File".into();
                    s.dialog_input.clear();
                    s.dialog_visible = true;
                    if let Some(a) = weak.upgrade() { a.set_dialog_input("".into()); }
                }
                "dialog:goto-symbol" => {
                    s.dialog_title = "Go to Symbol".into();
                    s.dialog_input.clear();
                    s.dialog_visible = true;
                    if let Some(a) = weak.upgrade() { a.set_dialog_input("".into()); }
                }
                "dialog:ok" => {
                    let cmd = weak.upgrade().map(|a| a.get_dialog_input().to_string()).unwrap_or_default();
                    if !cmd.trim().is_empty() {
                        match execute_command(&mut s.editor, &cmd, &HashMap::new()) {
                            Ok(()) => s.message.clear(),
                            Err(e) => s.message = e,
                        }
                    }
                    s.dialog_visible = false;
                    s.dialog_input.clear();
                    if let Some(a) = weak.upgrade() { a.set_dialog_input("".into()); }
                }
                _ => match execute_command(&mut s.editor, &command, &HashMap::new()) {
                    Ok(()) => s.message.clear(),
                    Err(e) => s.message = e,
                },
            }
            s.menu_visible = false;
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }


    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_toggle_bottom(move |index| {
            let mut s = state.borrow_mut();
            if s.bottom == index {
                s.bottom = -1;
            } else {
                s.bottom = index;
                if index == 0 {
                    let _ = execute_command(&mut s.editor, "terminal-start", &HashMap::new());
                }
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_close_bottom(move || {
            let mut s = state.borrow_mut();
            s.bottom = -1;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_focus(move |i| {
            let mut s = state.borrow_mut();
            if let Some(sess) = s.editor.io.terminals.sessions.get(i as usize) {
                s.editor.io.terminals.focused = Some(sess.id);
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_scroll(move |d| {
            let mut s = state.borrow_mut();
            let delta = (d / 16.0) as i32;
            if delta > 0 {
                s.terminal_scroll = s.terminal_scroll.saturating_add(delta as usize);
            } else {
                s.terminal_scroll = s.terminal_scroll.saturating_sub((-delta) as usize);
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_submit(move |text| {
            let mut s = state.borrow_mut();
            let term_id = s.editor.io.terminals.focused
                .or_else(|| s.editor.io.terminals.sessions.first().map(|t| t.id));
            if let Some(id) = term_id {
                let mut args = HashMap::new();
                args.insert("terminal".to_string(), ArgValue::Integer(id as i64));
                args.insert("text".to_string(), ArgValue::String(text.to_string() + "\n"));
                let _ = execute_command(&mut s.editor, "terminal-send-input", &args);
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_terminal_new(move || {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, "terminal-start", &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            s.bottom = 0;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_find_open(move |path, line| {
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, "open-file", &args) {
                Ok(()) => {
                    let mut args = HashMap::new();
                    args.insert("line".to_string(), ArgValue::Integer(line as i64));
                    if let Err(e) = execute_command(&mut s.editor, "goto-line", &args) {
                        s.message = e;
                    }
                }
                Err(e) => s.message = e,
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_debug_cmd(move |cmd| {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, &cmd, &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_toggle(move || {
            let mut s = state.borrow_mut();
            s.agent_open = !s.agent_open;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_submit(move || {
            let prompt = weak
                .upgrade()
                .map(|a| a.get_agent_prompt().to_string())
                .unwrap_or_default();
            if prompt.trim().is_empty() {
                return;
            }
            if let Some(app) = weak.upgrade() {
                app.set_agent_prompt("".into());
            }
            let mut s = state.borrow_mut();
            match magma::kernel::agent::loop_::start_session(&mut s.editor, &prompt) {
                Ok(id) => {
                    s.message = format!("agent session {id} started");
                    s.agent_open = true;
                }
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_agent_stop(move || {
            let mut s = state.borrow_mut();
            if let Some(id) = projection::running_session(&s.editor) {
                s.editor.agent.request_stop(id);
                s.message = "agent will stop after its current step".into();
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_change_clicked(move |path, staged| {
            let mut s = state.borrow_mut();
            let command = if staged { "vc-unstage-file" } else { "vc-stage-file" };
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, command, &args) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_commit(move || {
            let message = weak
                .upgrade()
                .map(|a| a.get_commit_message().to_string())
                .unwrap_or_default();
            if message.trim().is_empty() {
                return;
            }
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("message".to_string(), ArgValue::String(message));
            match execute_command(&mut s.editor, "vc-commit-message", &args) {
                Ok(()) => {
                    s.message.clear();
                    if let Some(app) = weak.upgrade() {
                        app.set_commit_message("".into());
                    }
                }
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_sidebar_refresh(move || {
            let mut s = state.borrow_mut();
            match execute_command(&mut s.editor, "index-project", &HashMap::new()) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_sidebar_collapse(move || {
            let mut s = state.borrow_mut();
            s.expanded.clear();
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_open_recent(move |path| {
            let mut s = state.borrow_mut();
            let mut args = HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path.to_string()));
            match execute_command(&mut s.editor, "open-project", &args) {
                Ok(()) => s.message.clear(),
                Err(e) => s.message = e,
            }
            s.tree_epoch = u64::MAX;
            refresh(&weak, &mut s);
        });
    }

    // ── Editor pane mouse ──────────────────────────────────────────────────
    {
        use magma::kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_pressed_left(move |x, y, ctrl, shift, alt, _meta| {
            let mut s = state.borrow_mut();
            let cell_x = (x / pane::CHAR_W as f32).max(0.0) as u16;
            let cell_y = (y / pane::LINE_H as f32).max(0.0) as u16;
            let mut mods = Vec::new();
            if ctrl { mods.push("ctrl"); }
            if shift { mods.push("shift"); }
            if alt { mods.push("alt"); }
            let ev = MouseEvent {
                kind: MouseEventKind::Click,
                x: cell_x,
                y: cell_y,
                button: MouseButton::Left,
                modifiers: mods.join(","),
            };
            let dummy_surface = magma::kernel::render::surface::Surface::new(1, 1);
            magma::kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        use magma::kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_pressed_right(move |x, y| {
            let mut s = state.borrow_mut();
            let cell_x = (x / pane::CHAR_W as f32).max(0.0) as u16;
            let cell_y = (y / pane::LINE_H as f32).max(0.0) as u16;
            let ev = MouseEvent {
                kind: MouseEventKind::Click,
                x: cell_x,
                y: cell_y,
                button: MouseButton::Right,
                modifiers: String::new(),
            };
            let dummy_surface = magma::kernel::render::surface::Surface::new(1, 1);
            magma::kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        use magma::kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_pressed_middle(move |x, y| {
            let mut s = state.borrow_mut();
            let cell_x = (x / pane::CHAR_W as f32).max(0.0) as u16;
            let cell_y = (y / pane::LINE_H as f32).max(0.0) as u16;
            let ev = MouseEvent {
                kind: MouseEventKind::Click,
                x: cell_x,
                y: cell_y,
                button: MouseButton::Middle,
                modifiers: String::new(),
            };
            let dummy_surface = magma::kernel::render::surface::Surface::new(1, 1);
            magma::kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_ime_committed(move |text| {
            let mut s = state.borrow_mut();
            magma::kernel::input::dispatch_key(&mut s.editor, &text);
            s.editor.dispatch_events_scoped();
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_hover(move |x, y| {
            let mut s = state.borrow_mut();
            s.last_px = x;
            s.last_py = y;
            s.hovering = true;
            let _ = weak;
        });
    }
    {
        use magma::kernel::input::event::{MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_moved(move |x, y| {
            let mut s = state.borrow_mut();
            if !s.editor.mouse_state.pressed { return; }
            let cell_x = (x / pane::CHAR_W as f32).max(0.0) as u16;
            let cell_y = (y / pane::LINE_H as f32).max(0.0) as u16;
            let button = s.editor.mouse_state.button.clone();
            let ev = MouseEvent {
                kind: MouseEventKind::Drag,
                x: cell_x,
                y: cell_y,
                button,
                modifiers: String::new(),
            };
            let dummy_surface = magma::kernel::render::surface::Surface::new(1, 1);
            magma::kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        use magma::kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_released(move || {
            let mut s = state.borrow_mut();
            let ev = MouseEvent {
                kind: MouseEventKind::Release,
                x: 0,
                y: 0,
                button: MouseButton::Left,
                modifiers: String::new(),
            };
            let dummy_surface = magma::kernel::render::surface::Surface::new(1, 1);
            magma::kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        use magma::kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_scrolled(move |x, y, dx, dy, ctrl, shift, alt, _meta| {
            let mut s = state.borrow_mut();
            let cell_x = (x / pane::CHAR_W as f32).max(0.0) as u16;
            let cell_y = (y / pane::LINE_H as f32).max(0.0) as u16;
            let mut mods = Vec::new();
            if ctrl { mods.push("ctrl"); }
            if shift { mods.push("shift"); }
            if alt { mods.push("alt"); }
            let mods = mods.join(",");
            let v_delta = (dy / pane::LINE_H as f32) as i32;
            if v_delta != 0 {
                let ev = MouseEvent {
                    kind: MouseEventKind::Scroll(v_delta),
                    x: cell_x,
                    y: cell_y,
                    button: MouseButton::Left,
                    modifiers: mods.clone(),
                };
                let dummy_surface = magma::kernel::render::surface::Surface::new(1, 1);
            magma::kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            }
            let h_delta = (dx / pane::CHAR_W as f32) as i32;
            if h_delta != 0 {
                let ev = MouseEvent {
                    kind: MouseEventKind::ScrollX(h_delta),
                    x: cell_x,
                    y: cell_y,
                    button: MouseButton::Left,
                    modifiers: mods,
                };
                let dummy_surface = magma::kernel::render::surface::Surface::new(1, 1);
            magma::kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            }
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }

}
