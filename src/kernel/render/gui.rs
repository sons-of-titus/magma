//! Graphical renderer using eframe / egui — native IDE layout.

use std::sync::{Arc, RwLock};

use eframe::egui::{self, FontFamily, FontId};
use tokio::sync::mpsc;

use crate::kernel::input::{self, NON_INSERTABLE};
use crate::kernel::input::event::{MouseEvent, MouseEventKind, MouseButton};
use crate::kernel::input::mouse;
use crate::kernel::render::frame::render_frame;
use crate::kernel::render::gpu_atlas::{build_glyph_instances, build_rect_instances, GpuGlyphAtlas};
use crate::kernel::render::gpu_paint_callback::MagmaPaintCallback;
use crate::kernel::render::surface::Surface;
use crate::kernel::runtime::{process_background_event, BackgroundEvent};
use crate::kernel::state::Editor;

use super::gui_fonts::apply_fonts_to_egui;
use super::gui_layout::GuiLayout;
use super::gui_render::present;
use super::gui_sidebar::{ProjectTree, SidebarState};
use super::gui_status::{StatusBar, STATUS_BAR_BG};
use super::gui_tabs::{TabAction, TabBar};
use super::tool_window::ToolWindowManager;

pub struct GuiApp {
    editor:          Arc<RwLock<Editor>>,
    font_size:       f32,
    layout:          GuiLayout,
    sidebar_state:   SidebarState,
    bg_receiver:     mpsc::UnboundedReceiver<BackgroundEvent>,
    atlas:           GpuGlyphAtlas,
    /// True once the atlas has been rasterized with real font data.
    atlas_rasterized: bool,
    /// True when `atlas_pixels` must be re-uploaded to the GPU this frame.
    atlas_dirty:     bool,
    target_format:   Option<eframe::wgpu::TextureFormat>,
    tool_windows:    ToolWindowManager,
}

impl GuiApp {
    pub fn new(editor: Arc<RwLock<Editor>>) -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self::init(editor, rx)
    }

    pub fn new_with_bg(editor: Arc<RwLock<Editor>>, bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        Self::init(editor, bg_receiver)
    }

    fn init(editor: Arc<RwLock<Editor>>, bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        let (font_size, font_bytes) = {
            let ed = editor.read().unwrap_or_else(|e| e.into_inner());
            (ed.font_config.size, ed.font_config.loaded_fonts.values().next().cloned())
        };
        let mut atlas = GpuGlyphAtlas::new();
        let atlas_rasterized = if let Some(bytes) = font_bytes {
            atlas.rasterize(&bytes, font_size);
            true
        } else {
            false
        };
        Self {
            editor,
            font_size,
            layout: GuiLayout::default(),
            sidebar_state: SidebarState::default(),
            bg_receiver,
            atlas,
            atlas_rasterized,
            atlas_dirty: true,
            target_format: None,
            tool_windows: ToolWindowManager::new(),
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // ── Background events ──────────────────────────────────────────────
        while let Ok(event) = self.bg_receiver.try_recv() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            process_background_event(&mut ed, event);
        }

        // ── GPU init + font rebuild ────────────────────────────────────────
        if let Some(rs) = frame.wgpu_render_state() {
            if self.target_format.is_none() {
                self.target_format = Some(rs.target_format);
            }
            let (needs_rebuild, font_config) = {
                let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
                let changed = ed.font_changed;
                ed.font_changed = false;
                (changed, ed.font_config.clone())
            };
            if needs_rebuild {
                apply_fonts_to_egui(ctx, &font_config);
                if let Some(bytes) = font_config.loaded_fonts.values().next() {
                    self.atlas.rasterize(bytes, self.font_size);
                    self.atlas_rasterized = true;
                    self.atlas_dirty = true;
                }
            }
        }

        // ── Font metrics (must precede input collection for MouseWheel unit conversion) ─
        self.font_size = {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            ed.font_config.size
        };
        let font_id = FontId::new(self.font_size, FontFamily::Monospace);
        let (char_w, line_h) = ctx.fonts(|f| (
            f.glyph_width(&font_id, 'M'),
            f.row_height(&font_id),
        ));

        // ── Key + mouse events ──────────────────────────────────────────────
        let mut key_events: Vec<String> = Vec::new();
        let mut raw_mouse_events: Vec<RawMouseEvent> = Vec::new();
        ctx.input(|i| {
            for ev in &i.events {
                if let Some(s) = translate_event(ev) {
                    key_events.push(s);
                }
                match ev {
                    egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                        let mb = match button {
                            egui::PointerButton::Primary => MouseButton::Left,
                            egui::PointerButton::Secondary => MouseButton::Right,
                            egui::PointerButton::Middle => MouseButton::Middle,
                            _ => MouseButton::Left,
                        };
                        let mods = format_modifiers(modifiers);
                        let kind = if *pressed { MouseEventKind::Click } else { MouseEventKind::Release };
                        raw_mouse_events.push(RawMouseEvent { px: pos.x, py: pos.y, kind, button: mb, modifiers: mods });
                    }
                    egui::Event::PointerMoved(pos) if i.pointer.any_down() => {
                        raw_mouse_events.push(RawMouseEvent { px: pos.x, py: pos.y, kind: MouseEventKind::Drag, button: MouseButton::Left, modifiers: String::new() });
                    }
                    egui::Event::MouseWheel { unit, delta, modifiers } => {
                        let dy = match unit {
                            egui::MouseWheelUnit::Point => (delta.y / line_h).round() as i32,
                            egui::MouseWheelUnit::Line => delta.y.round() as i32,
                            egui::MouseWheelUnit::Page => delta.y.round() as i32,
                        };
                        if dy != 0 {
                            let mods = format_modifiers(modifiers);
                            raw_mouse_events.push(RawMouseEvent { px: 0.0, py: 0.0, kind: MouseEventKind::Scroll(dy), button: MouseButton::Left, modifiers: mods });
                        }
                    }
                    _ => {}
                }
            }
        });
        {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            for key in &key_events {
                if key == "ctrl-q"  { ed.running = false; break; }
                if key == "ctrl-\\" { self.layout.sidebar_open = !self.layout.sidebar_open; continue; }
                input::dispatch_key(&mut ed, key);
            }
        }

        // ── Running check ─────────────────────────────────────────────────
        {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            if !ed.running {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        let bg_color = {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            let (r, g, b) = ed.theme_color("gui-bg");
            egui::Color32::from_rgb(r, g, b)
        };

        // ── Panel layout ──────────────────────────────────────────────────
        let ea             = Arc::clone(&self.editor);
        let sidebar_open   = self.layout.sidebar_open;
        let sidebar_width  = self.layout.sidebar_width;
        let status_h       = line_h + 8.0;
        let mut tab_action:   Option<TabAction> = None;
        let mut sidebar_file: Option<String>    = None;

        // Status bar — outermost bottom panel.
        egui::TopBottomPanel::bottom("magma_status")
            .exact_height(status_h)
            .frame(egui::Frame::none().fill(STATUS_BAR_BG).inner_margin(egui::Margin::symmetric(6.0, 0.0)))
            .show(ctx, |ui| {
                let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                StatusBar::show(ui, &ed);
            });

        // Docked-bottom tool windows — inside the status bar, outside the editor.
        {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            self.tool_windows.show_docked_bottom(ctx, &ed);
        }

        // Tab bar — top panel.
        egui::TopBottomPanel::top("magma_tabs")
            .show(ctx, |ui| {
                let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                if let Some(a) = TabBar::show(ui, &ed) {
                    tab_action = Some(a);
                }
            });

        // Tool window icon strip — outermost left strip (28 px wide).
        let mut toggled_tool: Option<usize> = None;
        egui::SidePanel::left("magma_tool_strip")
            .exact_width(28.0)
            .resizable(false)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(17, 17, 27))
                .inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| {
                toggled_tool = self.tool_windows.show_sidebar_edge(ui);
            });
        if let Some(idx) = toggled_tool {
            self.tool_windows.toggle(idx);
        }

        // Project tree sidebar.
        if sidebar_open {
            egui::SidePanel::left("magma_sidebar")
                .default_width(sidebar_width)
                .resizable(true)
                .show(ctx, |ui| {
                    let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                    sidebar_file = ProjectTree::show(ui, &ed, &mut self.sidebar_state);
                });
        }

        // Apply tab action before rendering the central editor pane.
        if let Some(action) = tab_action.take() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            action.apply(&mut ed);
        }

        // Open file requested by sidebar double-click.
        if let Some(path) = sidebar_file {
            use crate::kernel::command::{execute_command, args::ArgValue};
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            let mut args = std::collections::HashMap::new();
            args.insert("path".to_string(), ArgValue::Path(path));
            let _ = execute_command(&mut ed, "open-file", &args);
        }

        // ── Central panel: surface-based editor ───────────────────────────
        let use_gpu = self.target_format.is_some() && self.atlas_rasterized;
        let target_fmt = self.target_format;

        // Drain the dirty flag exactly once per frame.
        let atlas_pixels_upload = if self.atlas_dirty {
            self.atlas_dirty = false;
            Some(self.atlas.generate_atlas_pixels().to_vec())
        } else {
            None
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg_color))
            .show(ctx, |ui| {
                let full = ui.available_rect_before_wrap();
                let visible_cols = ((full.width()  / char_w).floor() as usize).max(1);
                let visible_rows = ((full.height() / line_h).floor() as usize).max(1);
                let mut surface = Surface::new(visible_cols as u16, visible_rows as u16);
                {
                    let mut ed = ea.write().unwrap_or_else(|e| e.into_inner());
                    ed.view_tree.resize(visible_cols as u16, visible_rows as u16);
                }
                {
                    let ed = ea.read().unwrap_or_else(|e| e.into_inner());
                    render_frame(&ed, &mut surface, true);
                }

                if use_gpu && let Some(fmt) = target_fmt {
                    let bg = [
                        bg_color.r() as f32 / 255.0,
                        bg_color.g() as f32 / 255.0,
                        bg_color.b() as f32 / 255.0,
                        1.0,
                    ];
                    let glyphs = build_glyph_instances(
                        &surface, &self.atlas, char_w, line_h,
                        full.min.x, full.min.y, true,
                    );
                    let rects = build_rect_instances(
                        &surface, char_w, line_h,
                        full.min.x, full.min.y, bg, true,
                    );
                    let callback = MagmaPaintCallback {
                        glyphs,
                        rects,
                        char_w,
                        line_h,
                        target_format: fmt,
                        atlas_width:   self.atlas.atlas_width,
                        atlas_height:  self.atlas.atlas_height,
                        atlas_pixels:  atlas_pixels_upload,
                    };
                    ui.painter_at(full).add(callback.into_paint_callback(full));
                } else {
                    present(ui, &ea, &surface, full, char_w, line_h, 0);
                }

                // ── Mouse dispatch (pixel → cell coords) ──────────────
                if !raw_mouse_events.is_empty() {
                    let mut ed = ea.write().unwrap_or_else(|e| e.into_inner());
                    for raw in raw_mouse_events.drain(..) {
                        let cell_x = ((raw.px - full.min.x) / char_w).max(0.0) as u16;
                        let cell_y = ((raw.py - full.min.y) / line_h).max(0.0) as u16;
                        let me = MouseEvent {
                            kind: raw.kind,
                            x: cell_x,
                            y: cell_y,
                            button: raw.button,
                            modifiers: raw.modifiers,
                        };
                        mouse::dispatch_mouse(&mut ed, &surface, &me);
                    }
                }
            });

        // Floating tool windows — rendered on top of everything.
        {
            let ed = ea.read().unwrap_or_else(|e| e.into_inner());
            self.tool_windows.show_floating(ctx, &ed);
        }

        ctx.request_repaint();
    }
}

/// A mouse event in pixel coordinates, converted to cell coords
/// inside the CentralPanel closure.
struct RawMouseEvent {
    px: f32,
    py: f32,
    kind: MouseEventKind,
    button: MouseButton,
    modifiers: String,
}

fn format_modifiers(mods: &egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if mods.ctrl || mods.command { parts.push("ctrl"); }
    if mods.shift { parts.push("shift"); }
    if mods.alt { parts.push("alt"); }
    parts.join(",")
}

fn translate_event(ev: &egui::Event) -> Option<String> {
    match ev {
        egui::Event::Key { key, pressed: true, modifiers, .. } => {
            use egui::Key;
            let ctrl  = modifiers.ctrl || modifiers.command;
            let alt   = modifiers.alt;
            let shift = modifiers.shift;

            let special: Option<&str> = match key {
                Key::Escape    => Some("esc"),
                Key::Backspace => Some("backspace"),
                Key::Enter     => Some("return"),
                Key::Tab       => Some("tab"),
                Key::Delete    => Some("delete"),
                Key::ArrowLeft  => Some("left"),
                Key::ArrowRight => Some("right"),
                Key::ArrowUp    => Some("up"),
                Key::ArrowDown  => Some("down"),
                Key::Home      => Some("home"),
                Key::End       => Some("end"),
                Key::PageUp    => Some("page-up"),
                Key::PageDown  => Some("page-down"),
                Key::F1  => Some("f1"),  Key::F2  => Some("f2"),
                Key::F3  => Some("f3"),  Key::F4  => Some("f4"),
                Key::F5  => Some("f5"),  Key::F6  => Some("f6"),
                Key::F7  => Some("f7"),  Key::F8  => Some("f8"),
                Key::F9  => Some("f9"),  Key::F10 => Some("f10"),
                Key::F11 => Some("f11"), Key::F12 => Some("f12"),
                _ => None,
            };
            if let Some(s) = special { return Some(s.to_string()); }

            if ctrl || alt {
                if *key == Key::Backslash {
                    let prefix = if ctrl { "ctrl" } else { "meta" };
                    return Some(format!("{}-\\", prefix));
                }
                let ch = key_char(*key)?;
                let prefix = if ctrl { "ctrl" } else { "meta" };
                return Some(format!("{}-{}", prefix, ch));
            }

            if !ctrl && !alt
                && let Some(c) = key_char(*key) {
                    if shift && !c.is_ascii_alphabetic() { return None; }
                    let out = if shift { c.to_uppercase().next().unwrap_or(c) } else { c };
                    return Some(out.to_string());
                }

            None
        }

        egui::Event::Text(text) if !text.is_empty() => {
            if text.chars().any(|c| c.is_control()) { return None; }
            if text.chars().count() == 1 {
                let c = text.chars().next()?;
                if c.is_ascii_alphanumeric() { return None; }
            }
            if NON_INSERTABLE.contains(&text.as_str()) { return None; }
            Some(text.clone())
        }

        _ => None,
    }
}

fn key_char(key: egui::Key) -> Option<char> {
    use egui::Key::*;
    match key {
        A => Some('a'), B => Some('b'), C => Some('c'), D => Some('d'),
        E => Some('e'), F => Some('f'), G => Some('g'), H => Some('h'),
        I => Some('i'), J => Some('j'), K => Some('k'), L => Some('l'),
        M => Some('m'), N => Some('n'), O => Some('o'), P => Some('p'),
        Q => Some('q'), R => Some('r'), S => Some('s'), T => Some('t'),
        U => Some('u'), V => Some('v'), W => Some('w'), X => Some('x'),
        Y => Some('y'), Z => Some('z'),
        Num0 => Some('0'), Num1 => Some('1'), Num2 => Some('2'),
        Num3 => Some('3'), Num4 => Some('4'), Num5 => Some('5'),
        Num6 => Some('6'), Num7 => Some('7'), Num8 => Some('8'),
        Num9 => Some('9'),
        _ => None,
    }
}
