//! Editor pane mouse plumbing: clicks, drags, wheel, hover, and IME commit
//! translated into kernel mouse events on the cell grid.

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
            // Right-click is also the editor's context menu: cursor first,
            // then the menu under the pointer.
            crate::actions::context_menu::raise_editor_menu(&weak, &mut s, x, y);
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
