//! Callback wiring: Slint signals → commands and state changes.
//!
//! Every component callback lands here.  Actions go through the command
//! registry whenever one exists, so the Slint shell behaves exactly like the
//! keymap, the menu, and the egui chrome.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use kernel::render::tabs;
use slint::ComponentHandle;

use crate::{input, logical_size, pane, projection, refresh, AppState, AppWindow};

pub(crate) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_key(move |text, ctrl, shift, alt, meta| {
            let Some(key) = input::translate_key(&text, ctrl, shift, alt, meta) else { return };
            let mut s = state.borrow_mut();
            kernel::input::dispatch_key(&mut s.editor, &key);
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
                    if let Some(a) = weak.upgrade() {
                        a.set_settings_visible(true);
                        a.set_settings_app_font_name(s.app_font_name.clone().into());
                        a.set_settings_app_font_size(s.app_font_size.to_string().into());
                        a.set_settings_editor_font_name(s.editor.font_config.family.clone().into());
                        a.set_settings_editor_font_size(s.editor.font_config.size.to_string().into());
                        a.set_settings_ligatures(s.editor.font_config.ligatures);
                    }
                }
                c if c.starts_with("settings:theme:") => {
                    let name = c.trim_start_matches("settings:theme:");
                    s.editor.options.insert("gui-theme".into(), name.into());
                    s.theme_name.clear();
                }
                // Font sizing is handled by apply-font-settings now.
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
                if index == 0 && s.editor.io.terminals.sessions.is_empty() {
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
        app.on_terminal_close(move |i| {
            let mut s = state.borrow_mut();
            let id = s.editor.io.terminals.sessions.get(i as usize).map(|t| t.id);
            if let Some(id) = id {
                let mut args = HashMap::new();
                args.insert("terminal".to_string(), ArgValue::Integer(id as i64));
                match execute_command(&mut s.editor, "terminal-close", &args) {
                    Ok(()) => s.message.clear(),
                    Err(e) => s.message = e,
                }
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
            match kernel::agent::loop_::start_session(&mut s.editor, &prompt) {
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
        use kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_pressed_left(move |x, y, ctrl, shift, alt, _meta| {
            let mut s = state.borrow_mut();
            let cell_x = (x / s.cell_w as f32).max(0.0) as u16;
            let cell_y = (y / s.cell_h as f32).max(0.0) as u16;
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
            let dummy_surface = kernel::render::surface::Surface::new(1, 1);
            kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        use kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_pressed_right(move |x, y| {
            let mut s = state.borrow_mut();
            let cell_x = (x / s.cell_w as f32).max(0.0) as u16;
            let cell_y = (y / s.cell_h as f32).max(0.0) as u16;
            let ev = MouseEvent {
                kind: MouseEventKind::Click,
                x: cell_x,
                y: cell_y,
                button: MouseButton::Right,
                modifiers: String::new(),
            };
            let dummy_surface = kernel::render::surface::Surface::new(1, 1);
            kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        use kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_mouse_pressed_middle(move |x, y| {
            let mut s = state.borrow_mut();
            let cell_x = (x / s.cell_w as f32).max(0.0) as u16;
            let cell_y = (y / s.cell_h as f32).max(0.0) as u16;
            let ev = MouseEvent {
                kind: MouseEventKind::Click,
                x: cell_x,
                y: cell_y,
                button: MouseButton::Middle,
                modifiers: String::new(),
            };
            let dummy_surface = kernel::render::surface::Surface::new(1, 1);
            kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_ime_committed(move |text| {
            let mut s = state.borrow_mut();
            kernel::input::dispatch_key(&mut s.editor, &text);
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
        use kernel::input::event::{MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        app.on_mouse_moved(move |x, y| {
            let mut s = state.borrow_mut();
            if !s.editor.mouse_state.pressed { return; }
            let cell_x = (x / s.cell_w as f32).max(0.0) as u16;
            let cell_y = (y / s.cell_h as f32).max(0.0) as u16;
            let button = s.editor.mouse_state.button.clone();
            let ev = MouseEvent {
                kind: MouseEventKind::Drag,
                x: cell_x,
                y: cell_y,
                button,
                modifiers: String::new(),
            };
            let dummy_surface = kernel::render::surface::Surface::new(1, 1);
            kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            // Let the frame timer coalesce drag updates: a full pane blit per
            // motion event stalls the UI thread.
            s.editor.frame_dirty = true;
        });
    }
    {
        use kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
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
            let dummy_surface = kernel::render::surface::Surface::new(1, 1);
            kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        use kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
        let state = Rc::clone(&state);
        app.on_mouse_scrolled(move |x, y, dx, dy, ctrl, shift, alt, _meta| {
            let mut s = state.borrow_mut();
            let cell_x = (x / s.cell_w as f32).max(0.0) as u16;
            let cell_y = (y / s.cell_h as f32).max(0.0) as u16;
            let mut mods = Vec::new();
            if ctrl { mods.push("ctrl"); }
            if shift { mods.push("shift"); }
            if alt { mods.push("alt"); }
            let mods = mods.join(",");

            // Trackpad deltas are sub-line: accumulate them so a slow
            // two-finger scroll still advances by whole cell rows.
            let (raw_x, raw_y) = if shift && dx == 0.0 {
                (dy, 0.0)
            } else {
                (dx, dy)
            };
            s.wheel_y += raw_y;
            s.wheel_x += raw_x;
            let v_delta = (s.wheel_y / s.cell_h as f32).trunc() as i32;
            let h_delta = (s.wheel_x / s.cell_w as f32).trunc() as i32;
            s.wheel_y -= v_delta as f32 * s.cell_h as f32;
            s.wheel_x -= h_delta as f32 * s.cell_w as f32;

            if v_delta != 0 {
                let ev = MouseEvent {
                    kind: MouseEventKind::Scroll(v_delta),
                    x: cell_x,
                    y: cell_y,
                    button: MouseButton::Left,
                    modifiers: mods.clone(),
                };
                let dummy_surface = kernel::render::surface::Surface::new(1, 1);
                kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            }
            if h_delta != 0 {
                let ev = MouseEvent {
                    kind: MouseEventKind::ScrollX(h_delta),
                    x: cell_x,
                    y: cell_y,
                    button: MouseButton::Left,
                    modifiers: mods,
                };
                let dummy_surface = kernel::render::surface::Surface::new(1, 1);
                kernel::input::mouse::dispatch_mouse(&mut s.editor, &dummy_surface, &ev);
            }
            // Coalesce with any further wheel events; the frame timer redraws.
            s.editor.frame_dirty = true;
        });
    }

    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_apply_font_settings(move || {
            let Some(app) = weak.upgrade() else { return; };
            let app_font_name = app.get_settings_app_font_name().to_string();
            let app_font_size: f32 = app.get_settings_app_font_size().parse::<f32>().unwrap_or(12.0).max(6.0);
            let editor_font_name = app.get_settings_editor_font_name().to_string();
            let editor_font_size: f32 = app.get_settings_editor_font_size().parse::<f32>().unwrap_or(15.0).max(4.0);
            let ligatures = app.get_settings_ligatures();

            let mut s = state.borrow_mut();
            s.app_font_name = app_font_name.clone();
            s.app_font_size = app_font_size;
            if let Some(a) = weak.upgrade() {
                let g = a.global::<crate::Theme>();
                g.set_font_ui(app_font_size);
                g.set_font_small((app_font_size - 1.0).max(8.0));
                a.set_app_font_name(s.app_font_name.clone().into());
                a.set_app_font_size(app_font_size);
            }

            if !editor_font_name.trim().is_empty() {
                if !s.editor.font_config.loaded_fonts.contains_key(&editor_font_name) {
                    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
                    if let Some(home) = std::env::var_os("HOME") {
                        let dir = std::path::PathBuf::from(home).join("Library/Fonts");
                        if let Ok(entries) = std::fs::read_dir(&dir) {
                            for e in entries.flatten() {
                                let p = e.path();
                                if let Some(ext) = p.extension().and_then(|x| x.to_str())
                                    && (ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf")) {
                                        candidates.push(p);
                                    }
                            }
                        }
                    }
                    for dir in ["/System/Library/Fonts", "/Library/Fonts"] {
                        if let Ok(entries) = std::fs::read_dir(dir) {
                            for e in entries.flatten() {
                                let p = e.path();
                                if let Some(ext) = p.extension().and_then(|x| x.to_str())
                                    && (ext.eq_ignore_ascii_case("ttf") || ext.eq_ignore_ascii_case("otf")) {
                                        candidates.push(p);
                                    }
                            }
                        }
                    }
                    let needle = editor_font_name.to_lowercase();
                    for p in candidates {
                        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                        if stem.contains(&needle) || needle.contains(&stem) {
                            if let Ok(bytes) = std::fs::read(&p)
                                && bytes.len() > 1000 {
                                    s.editor.font_config.loaded_fonts.insert(editor_font_name.clone(), bytes);
                                    break;
                                }
                        }
                    }
                }
                s.editor.font_config.family = editor_font_name;
            }
            s.editor.font_config.size = editor_font_size;
            s.editor.font_config.ligatures = ligatures;
            s.editor.font_changed = true;

            if let Some(bytes) = s.editor.font_config.primary_font().map(|b| b.clone()) {
                let (cw, ch) = pane::measure_cell(&bytes, editor_font_size);
                s.cell_w = cw.max(1);
                s.cell_h = ch.max(1);
            }

            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
}
