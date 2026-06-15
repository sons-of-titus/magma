//! Integration test: verify the render-frame Janet handlers actually modify
//! the surface — confirming the full pipeline from SURFACE_PTR → C function →
//! Janet draw-modeline/draw-tab-bar is wired correctly.

#[cfg(feature = "janet")]
mod tests {
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::janet_bridge;
    use crate::render::surface::Surface;
    use crate::render::frame::render_frame;
    use crate::state::Editor;
    use crate::buffer::Buffer;
    use crate::state::id::BufferId;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        ed
    }

    /// Run the full render-frame pipeline with a real surface and confirm that
    /// the Janet UI handlers write to the surface.
    #[test]
    fn render_frame_event_modifies_surface() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();

        // Create *scratch* so it exists (matching production behaviour).
        let buf_key = ed.buffers.insert(Buffer::new(BufferId(1), "*scratch*"));
        if let Some(win) = ed.windows.focused_window_mut() {
            win.buffer_id = Some(buf_key);
        }

        janet_bridge::init(&mut ed);

        // Fire editor-ready so ui.janet sets tab_bar_enabled, line numbers, faces.
        let data = std::collections::HashMap::new();
        ed.events.emit("editor-ready", data.clone());
        ed.events.drain_and_dispatch();

        assert!(ed.tab_bar_enabled, "editor-ready should set tab_bar_enabled");
        assert!(ed.faces.contains_key("ml-normal"), "ml-normal face should be defined");

        // Create a surface and run render_frame.
        let mut surface = Surface::new(80, 24);
        render_frame(&ed, &mut surface);

        // Remember what the status bar looks like after Rust's render.
        let rust_status_row: String = (0..80)
            .filter_map(|x| surface.cell(x, 23))
            .map(|c| c.ch)
            .collect();

        // Now dispatch the render-frame event with SURFACE_PTR set.
        janet_bridge::set_surface_ptr(&mut surface as *mut _);
        ed.events.emit("render-frame", data.clone());
        if ed.tab_bar_enabled {
            ed.events.emit("render-tab-bar", data.clone());
        }
        ed.events.drain_and_dispatch();
        janet_bridge::clear_surface_ptr();

        // Check what row 23 (status bar) looks like after Janet ran.
        let janet_status_row: String = (0..80)
            .filter_map(|x| surface.cell(x, 23))
            .map(|c| c.ch)
            .collect();

        // Janet modeline must contain the mode name (pill: "▌ NORMAL ▌").
        assert!(
            janet_status_row.contains("NORMAL"),
            "Janet modeline should write mode name to status row.\n\
             Before (Rust): {:?}\n\
             After (Janet): {:?}",
            rust_status_row.trim(),
            janet_status_row.trim()
        );

        // Tab bar row (row 0) should have *scratch* in it.
        // (*scratch* IS the focused buffer — it passes user-buffer? filter
        // because it equals cur.)
        let tab_row: String = (0..80)
            .filter_map(|x| surface.cell(x, 0))
            .map(|c| c.ch)
            .collect();
        assert!(
            tab_row.contains("*scratch*"),
            "Tab bar row 0 should contain focused buffer name.\n\
             Tab row: {:?}",
            tab_row.trim()
        );
    }
}
