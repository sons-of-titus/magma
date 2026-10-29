//! Editor-surface wiring: keyboard, mouse, tabs, tree, rail, and the
//! command dispatcher shared by menus and toolbar buttons.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::args::ArgValue;
use kernel::command::execute_command;
use kernel::render::tabs;
use slint::ComponentHandle;

use super::settings;
use crate::{AppState, AppWindow, input, logical_size, projection, sync::refresh};

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_key(move |text, ctrl, shift, alt, meta| {
            let Some(key) = input::translate_key(&text, ctrl, shift, alt, meta) else {
                return;
            };
            let mut s = state.borrow_mut();
            // The New Project wizard is modal while it has no text field
            // focused: Escape closes, Return creates, arrows move the
            // template selection, everything else stays out of the buffer.
            if s.new_project.visible {
                match key.as_str() {
                    "esc" if !s.editor.template_jobs.is_generating() => {
                        crate::new_project::close(&mut s);
                    }
                    "return" | "enter"
                        if s.new_project.can_create && !s.editor.template_jobs.is_generating() =>
                    {
                        crate::new_project::command(&weak, &mut s, "new-project:create");
                        return;
                    }
                    "up" => {
                        crate::new_project::command(&weak, &mut s, "new-project:move:-1");
                        return;
                    }
                    "down" => {
                        crate::new_project::command(&weak, &mut s, "new-project:move:1");
                        return;
                    }
                    _ => {}
                }
                return;
            }
            // The unsaved-changes dialog is modal: Return saves, Escape
            // cancels, and everything else stays out of the buffer.
            if s.quit_confirm_visible {
                match key.as_str() {
                    "return" => crate::lifecycle::confirm_command(&weak, &mut s, "quit:save"),
                    "esc" => crate::lifecycle::confirm_command(&weak, &mut s, "quit:cancel"),
                    _ => {}
                }
                return;
            }
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
            if s.rail == index {
                s.sidebar_open = !s.sidebar_open;
            } else {
                s.rail = index;
                s.sidebar_open = true;
                if index == 1 {
                    let _ = execute_command(&mut s.editor, "vc-refresh", &HashMap::new());
                }
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
                    super::open_project_dialog(&mut s);
                }
                "toggle-theme" => {
                    let next = match s.theme_name.as_str() {
                        "intellij-dark" => "intellij-light",
                        "intellij-light" => "catppuccin-mocha",
                        "mocha" | "catppuccin-mocha" => "catppuccin-latte",
                        "latte" | "catppuccin-latte" => "monokai",
                        _ => "intellij-dark",
                    };
                    s.editor.options.insert("gui-theme".into(), next.into());
                    kernel::state::persist::save_settings(&s.editor);
                    s.theme_name.clear();
                }
                "rail:0" | "rail:1" | "rail:2" => {
                    s.rail = command.trim_start_matches("rail:").parse().unwrap_or(0);
                    if s.rail == 1 {
                        let _ = execute_command(&mut s.editor, "vc-refresh", &HashMap::new());
                    }
                }
                "debug-config" | "dbg-start" => {
                    s.rail = 2;
                    match execute_command(&mut s.editor, &command, &HashMap::new()) {
                        Ok(()) => s.message.clear(),
                        Err(e) => s.message = e,
                    }
                }
                "run-config" => {
                    s.bottom = 2;
                    match execute_command(&mut s.editor, &command, &HashMap::new()) {
                        Ok(()) => s.message.clear(),
                        Err(e) => s.message = e,
                    }
                }
                "new-project-dialog" => {
                    crate::new_project::open(&weak, &mut s);
                }
                c if c.starts_with("new-project:") => {
                    crate::new_project::command(&weak, &mut s, c);
                }
                c if c.starts_with("quit:") => {
                    crate::lifecycle::confirm_command(&weak, &mut s, c);
                }
                c if c.starts_with("settings:") || c.starts_with("dialog:") => {
                    settings::command(&weak, &mut s, &command);
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

    // ── Editor pane mouse ────────────────────────────────────────────────
    mouse::install(app, state);
}

mod mouse {
    use std::cell::RefCell;
    use std::rc::Rc;

    use kernel::input::event::{MouseButton, MouseEvent, MouseEventKind};
    use slint::ComponentHandle;

    use crate::{AppState, AppWindow, sync::refresh};

    fn cell_xy(s: &AppState, x: f32, y: f32) -> (u16, u16) {
        (
            (x / s.cell_w as f32).max(0.0) as u16,
            (y / s.cell_h as f32).max(0.0) as u16,
        )
    }

    pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
        {
            let state = Rc::clone(&state);
            let weak = app.as_weak();
            app.on_mouse_pressed_left(move |x, y, ctrl, shift, alt, _meta| {
                let mut s = state.borrow_mut();
                let (cell_x, cell_y) = cell_xy(&s, x, y);
                let mut mods = Vec::new();
                if ctrl {
                    mods.push("ctrl");
                }
                if shift {
                    mods.push("shift");
                }
                if alt {
                    mods.push("alt");
                }
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
            let state = Rc::clone(&state);
            let weak = app.as_weak();
            app.on_mouse_pressed_right(move |x, y| {
                let mut s = state.borrow_mut();
                let (cell_x, cell_y) = cell_xy(&s, x, y);
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
            let state = Rc::clone(&state);
            let weak = app.as_weak();
            app.on_mouse_pressed_middle(move |x, y| {
                let mut s = state.borrow_mut();
                let (cell_x, cell_y) = cell_xy(&s, x, y);
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
                if s.quit_confirm_visible || s.new_project.visible {
                    return;
                }
                kernel::input::dispatch_key(&mut s.editor, &text);
                s.editor.dispatch_events_scoped();
                s.editor.frame_dirty = true;
                refresh(&weak, &mut s);
            });
        }
        {
            let state = Rc::clone(&state);
            app.on_mouse_hover(move |x, y| {
                let mut s = state.borrow_mut();
                s.last_px = x;
                s.last_py = y;
                s.hovering = true;
            });
        }
        {
            let state = Rc::clone(&state);
            app.on_mouse_moved(move |x, y| {
                let mut s = state.borrow_mut();
                if !s.editor.mouse_state.pressed {
                    return;
                }
                let (cell_x, cell_y) = cell_xy(&s, x, y);
                if !s.editor.mouse_state.drag_active
                    && cell_x == s.editor.mouse_state.press_x
                    && cell_y == s.editor.mouse_state.press_y
                {
                    return;
                }
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
            let state = Rc::clone(&state);
            app.on_mouse_scrolled(move |x, y, dx, dy, ctrl, shift, alt, _meta| {
                let mut s = state.borrow_mut();
                let (cell_x, cell_y) = cell_xy(&s, x, y);
                let mut mods = Vec::new();
                if ctrl {
                    mods.push("ctrl");
                }
                if shift {
                    mods.push("shift");
                }
                if alt {
                    mods.push("alt");
                }
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
                // Coalesce with further wheel events: trackpads deliver 60–120
                // events/s and a synchronous pane blit per event stalls the
                // UI thread.  The frame timer redraws within one tick.
                s.editor.frame_dirty = true;
            });
        }
    }
}
