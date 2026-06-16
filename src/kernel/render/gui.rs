//! Graphical renderer using eframe / egui.

use std::sync::{Arc, RwLock};

use eframe::egui::{self, FontFamily, FontId, Key, Rect, Vec2, Pos2};
use tokio::sync::mpsc;

use crate::kernel::input::{self, NON_INSERTABLE};
use crate::kernel::render::frame::render_frame;
use crate::kernel::render::gpu_atlas::{build_glyph_instances, build_rect_instances, GpuGlyphAtlas};
use crate::kernel::render::gpu_paint_callback::MagmaPaintCallback;
use crate::kernel::render::surface::Surface;
use crate::kernel::runtime::{process_background_event, BackgroundEvent};
use crate::kernel::state::Editor;

use super::gui_fonts::apply_fonts_to_egui;
use super::gui_render::{present, draw_status};

pub struct GuiApp {
    editor: Arc<RwLock<Editor>>,
    font_size: f32,
    scroll_row: usize,
    bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>,
    atlas: GpuGlyphAtlas,
    gpu_path_ready: bool,
    target_format: Option<eframe::wgpu::TextureFormat>,
}

impl GuiApp {
    pub fn new(editor: Arc<RwLock<Editor>>) -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self {
            editor,
            font_size: 15.0,
            scroll_row: 0,
            bg_receiver: rx,
            atlas: GpuGlyphAtlas::new(),
            gpu_path_ready: false,
            target_format: None,
        }
    }

    pub fn new_with_bg(editor: Arc<RwLock<Editor>>, bg_receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        Self {
            editor,
            font_size: 15.0,
            scroll_row: 0,
            bg_receiver,
            atlas: GpuGlyphAtlas::new(),
            gpu_path_ready: false,
            target_format: None,
        }
    }
}

impl eframe::App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok(event) = self.bg_receiver.try_recv() {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            process_background_event(&mut ed, event);
        }

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
                self.atlas.mark_dirty();
                apply_fonts_to_egui(ctx, &font_config);
            }
        }

        let mut key_events: Vec<String> = Vec::new();
        ctx.input(|i| {
            for ev in &i.events {
                if let Some(s) = translate_event(ev) {
                    key_events.push(s);
                }
            }
        });

        {
            let mut ed = self.editor.write().unwrap_or_else(|e| e.into_inner());
            for key in &key_events {
                if key == "ctrl-q" { ed.running = false; break; }
                input::dispatch_key(&mut ed, key);
            }
        }

        {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            if !ed.running {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        }

        self.font_size = {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            ed.font_config.size
        };
        let font_id = FontId::new(self.font_size, FontFamily::Monospace);
        let (char_w, line_h) = ctx.fonts(|f| (
            f.glyph_width(&font_id, 'M'),
            f.row_height(&font_id),
        ));

        let status_h = line_h + 8.0;
        let approx_visible = {
            let h = ctx.screen_rect().height() - status_h;
            ((h / line_h).floor() as usize).max(1)
        };
        {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            if let Some(slab) = ed.view_tree.focused_window()
                .and_then(|wid| ed.view_tree.buffer(wid))
                && let Some(buf_arc) = ed.buffers.get(slab) {
                    let buf = buf_arc.lock().unwrap();
                    let cursor = ed.views.get(&slab).map(|v| v.cursor_offset()).unwrap_or(0);
                    let text = buf.slice(0, buf.len());
                    let safe = super::gui_render::safe_boundary(&text, cursor);
                    let crow = text[..safe].chars().filter(|&c| c == '\n').count();
                    if crow < self.scroll_row {
                        self.scroll_row = crow;
                    } else if crow + 1 > self.scroll_row + approx_visible {
                        self.scroll_row = crow + 1 - approx_visible;
                    }
                }
        }

        let font_ref    = &font_id;
        let editor_ref  = &self.editor;
        let bg_color = {
            let ed = self.editor.read().unwrap_or_else(|e| e.into_inner());
            let (r, g, b) = ed.theme_color("gui-bg");
            egui::Color32::from_rgb(r, g, b)
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(bg_color))
            .show(ctx, |ui| {
                let full = ui.available_rect_before_wrap();
                let text_h  = (full.height() - status_h).max(0.0);
                let visible_rows = ((text_h / line_h).floor() as usize).max(1);

                let text_rect = Rect::from_min_size(full.min, Vec2::new(full.width(), text_h));
                let status_rect = Rect::from_min_size(
                    Pos2::new(full.min.x, full.min.y + text_h),
                    Vec2::new(full.width(), status_h),
                );

                let visible_cols = (full.width() / char_w).floor() as usize;
                let surface_rows = visible_rows + 1;
                let mut surface = Surface::new(visible_cols as u16, surface_rows as u16);
                {
                    let ed = editor_ref.read().unwrap_or_else(|e| e.into_inner());
                    render_frame(&ed, &mut surface);
                }

                if self.gpu_path_ready
                    && let Some(fmt) = self.target_format
                {
                    use std::sync::Arc;
                    let bg = [
                        bg_color.r() as f32 / 255.0,
                        bg_color.g() as f32 / 255.0,
                        bg_color.b() as f32 / 255.0,
                        1.0,
                    ];
                    let glyphs = build_glyph_instances(
                        &surface, &self.atlas, char_w, line_h,
                        text_rect.min.x, text_rect.min.y, true,
                    );
                    let rects = build_rect_instances(
                        &surface, char_w, line_h,
                        text_rect.min.x, text_rect.min.y, bg, true,
                    );
                    let callback = MagmaPaintCallback {
                        glyphs,
                        rects,
                        char_w,
                        line_h,
                        target_format: fmt,
                        atlas: Arc::new(GpuGlyphAtlas::new()),
                    };
                    let painter = ui.painter_at(text_rect);
                    painter.add(callback.into_paint_callback(text_rect));
                } else {
                    present(ui, editor_ref, &surface, text_rect, char_w, line_h, 0);
                }

                draw_status(ui, editor_ref, status_rect, font_ref, line_h);
            });

        ctx.request_repaint();
    }
}

fn translate_event(ev: &egui::Event) -> Option<String> {
    match ev {
        egui::Event::Key { key, pressed: true, modifiers, .. } => {
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
                let ch = key_char(*key)?;
                let prefix = if ctrl { "ctrl" } else { "meta" };
                return Some(format!("{}-{}", prefix, ch));
            }

            if !ctrl && !alt
                && let Some(c) = key_char(*key) {
                    if shift && !c.is_ascii_alphabetic() {
                        return None;
                    }
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

fn key_char(key: Key) -> Option<char> {
    use Key::*;
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
