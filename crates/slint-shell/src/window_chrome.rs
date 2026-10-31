//! Frameless-window resize for the main editor window.
//!
//! The window is `no-frame`, and winit cannot drag-resize undecorated
//! windows on macOS (`drag_resize_window` is Windows/X11 only).  The
//! `WindowResizeEdges` overlay therefore reports the pressed direction and
//! the pointer delta, and the geometry is applied here through the window
//! API — identical behavior on every platform.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, LogicalPosition, LogicalSize};

use crate::AppWindow;

/// Directions are bit flags shared with `WindowResizeEdges`:
/// east 1, south 2, west 4, north 8.
const EAST: i32 = 0b0001;
const SOUTH: i32 = 0b0010;
const WEST: i32 = 0b0100;
const NORTH: i32 = 0b1000;

/// Logical minimums; keep in sync with AppWindow's `min-width`/`min-height`.
const MIN_WIDTH: f32 = 480.0;
const MIN_HEIGHT: f32 = 360.0;

#[derive(Clone, Copy)]
struct ResizeStart {
    width: f32,
    height: f32,
    x: f32,
    y: f32,
}

pub(super) fn install(app: &AppWindow) {
    let start: Rc<RefCell<Option<ResizeStart>>> = Rc::new(RefCell::new(None));

    let weak = app.as_weak();
    let press = Rc::clone(&start);
    app.on_window_resize_begin(move |_| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let scale = app.window().scale_factor();
        let size = app.window().size().to_logical(scale);
        let pos = app.window().position().to_logical(scale);
        *press.borrow_mut() = Some(ResizeStart {
            width: size.width,
            height: size.height,
            x: pos.x,
            y: pos.y,
        });
    });

    let weak = app.as_weak();
    let drag_start = Rc::clone(&start);
    app.on_window_resize_move(move |dir, dx, dy| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let Some(s) = *drag_start.borrow() else {
            return;
        };

        let mut width = s.width + if dir & EAST != 0 { dx } else { 0.0 }
            - if dir & WEST != 0 { dx } else { 0.0 };
        let mut height = s.height + if dir & SOUTH != 0 { dy } else { 0.0 }
            - if dir & NORTH != 0 { dy } else { 0.0 };
        width = width.max(MIN_WIDTH);
        height = height.max(MIN_HEIGHT);
        // West/north resizes anchor the opposite edge, so the origin moves by
        // whatever the size actually changed.
        let x = if dir & WEST != 0 {
            s.x + (s.width - width)
        } else {
            s.x
        };
        let y = if dir & NORTH != 0 {
            s.y + (s.height - height)
        } else {
            s.y
        };
        app.window().set_size(LogicalSize::new(width, height));
        if x != s.x || y != s.y {
            app.window().set_position(LogicalPosition::new(x, y));
        }
    });
}
