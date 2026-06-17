//! TUI renderer using crossterm and ratatui.

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
            EnableMouseCapture, DisableMouseCapture},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    Terminal,
};
use std::io::{stdout, Write};

use super::RenderTrait;
use crate::kernel::render::surface::Surface as MagmaSurface;

pub struct TuiRenderer {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    width: u16,
    height: u16,
    /// When a terminal sends `Char(base) + SHIFT` as the first of two events,
    /// `shift_char(base)` is stored here so we can drop the redundant
    /// "produced character" event that some terminals send next.
    pending_shift_absorb: Option<char>,
}

impl TuiRenderer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        stdout().execute(EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;
        let _ = stdout().execute(EnableMouseCapture);
        // Enable SGR encoding and button-event tracking for drag / extended coordinates.
        let _ = write!(stdout(), "\x1b[?1002h\x1b[?1006h");
        let _ = stdout().flush();
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        terminal.clear()?;
        let size = terminal.size()?;
        Ok(TuiRenderer {
            terminal,
            width: size.width,
            height: size.height,
            pending_shift_absorb: None,
        })
    }
}

impl Drop for TuiRenderer {
    fn drop(&mut self) {
        let _ = write!(stdout(), "\x1b[?1006l\x1b[?1002l");
        let _ = stdout().execute(DisableMouseCapture);
        let _ = terminal::disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}

impl RenderTrait for TuiRenderer {
    fn draw(&mut self, magma_surface: &MagmaSurface) {
        let _ = self.terminal.draw(|frame| {
            let area = frame.area();
            // Render every row of the Surface directly — the Surface already
            // contains the correct status bar on its last row (set by render_frame).
            // We do NOT split off the last row for a separate widget; doing so was
            // causing a hardcoded dummy bar to replace the real mode/filename/col line.
            let lines = render_surface(magma_surface);
            // Only render as many rows as the terminal actually has.
            let visible = (area.height as usize).min(lines.len());
            for (y, line) in lines.into_iter().take(visible).enumerate() {
                let row_area = ratatui::layout::Rect {
                    x: area.x,
                    y: area.y + y as u16,
                    width: area.width,
                    height: 1,
                };
                frame.render_widget(
                    ratatui::widgets::Paragraph::new(line),
                    row_area,
                );
            }
        });
    }

    fn poll_event(&mut self) -> Option<crate::kernel::input::event::InputEvent> {
        use crate::kernel::input::event::{InputEvent, MouseEvent, MouseEventKind as MagmaMouseKind, MouseButton as MagmaMouseButton};
        if event::poll(std::time::Duration::from_millis(50)).ok()? {
            match event::read().ok()? {
                Event::Key(key)
                    if key.kind == KeyEventKind::Press
                        || key.kind == KeyEventKind::Repeat =>
                {
                    // Detect the two-event pattern some terminals emit:
                    //   Event 1: Char(base) + SHIFT  (e.g. '9' with SHIFT)
                    //   Event 2: Char(produced)      (e.g. '(', often without SHIFT)
                    // After shift_char transforms event 1 to '(', event 2 is a
                    // duplicate.  We absorb it by checking if the produced char
                    // matches what shift_char already returned.
                    if let KeyCode::Char(c) = key.code {
                        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                        if !shift {
                            if self.pending_shift_absorb == Some(c) {
                                self.pending_shift_absorb = None;
                                return None; // drop duplicate
                            }
                            self.pending_shift_absorb = None;
                        } else {
                            let produced = crate::kernel::input::keys::shift_char(c);
                            if produced != c {
                                // This is a "base + SHIFT" event; the terminal may
                                // follow it with the produced character.  Prime the
                                // absorb slot so we can drop that duplicate.
                                self.pending_shift_absorb = Some(produced);
                            } else {
                                self.pending_shift_absorb = None;
                            }
                        }
                    } else {
                        self.pending_shift_absorb = None;
                    }
                    Some(InputEvent::Key(key_to_string(key)))
                }
                Event::Resize(w, h) => {
                    self.width = w;
                    self.height = h;
                    Some(InputEvent::Resize(w, h))
                }
                Event::Mouse(m) => {
                    let (kind, button) = match m.kind {
                        MouseEventKind::Down(btn) => {
                            let b = match btn {
                                MouseButton::Left => MagmaMouseButton::Left,
                                MouseButton::Right => MagmaMouseButton::Right,
                                MouseButton::Middle => MagmaMouseButton::Middle,
                            };
                            (MagmaMouseKind::Click, b)
                        }
                        MouseEventKind::Up(btn) => {
                            let b = match btn {
                                MouseButton::Left => MagmaMouseButton::Left,
                                MouseButton::Right => MagmaMouseButton::Right,
                                MouseButton::Middle => MagmaMouseButton::Middle,
                            };
                            (MagmaMouseKind::Release, b)
                        }
                        MouseEventKind::Drag(btn) => {
                            let b = match btn {
                                MouseButton::Left => MagmaMouseButton::Left,
                                MouseButton::Right => MagmaMouseButton::Right,
                                MouseButton::Middle => MagmaMouseButton::Middle,
                            };
                            (MagmaMouseKind::Drag, b)
                        }
                        MouseEventKind::ScrollDown => (MagmaMouseKind::Scroll(1), MagmaMouseButton::Left),
                        MouseEventKind::ScrollUp => (MagmaMouseKind::Scroll(-1), MagmaMouseButton::Left),
                        _ => return None,
                    };
                    let mut mods: Vec<&str> = Vec::new();
                    if m.modifiers.contains(KeyModifiers::SHIFT) { mods.push("shift"); }
                    if m.modifiers.contains(KeyModifiers::CONTROL) { mods.push("ctrl"); }
                    if m.modifiers.contains(KeyModifiers::ALT) { mods.push("alt"); }
                    Some(InputEvent::Mouse(MouseEvent {
                        kind,
                        x: m.column,
                        y: m.row,
                        button,
                        modifiers: mods.join(","),
                    }))
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn set_title(&mut self, title: &str) {
        let _ = stdout().execute(crossterm::terminal::SetTitle(title));
    }

    fn close(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}

fn render_surface<'a>(surface: &MagmaSurface) -> Vec<Line<'a>> {
    let mut lines = Vec::with_capacity(surface.height as usize);
    for y in 0..surface.height {
        let mut spans = Vec::new();
        for x in 0..surface.width {
            if let Some(cell) = surface.cell(x, y) {
                let style = Style::default()
                    .fg(Color::Rgb(cell.style.fg.0, cell.style.fg.1, cell.style.fg.2))
                    .bg(Color::Rgb(cell.style.bg.0, cell.style.bg.1, cell.style.bg.2));
                let mut style = style;
                if cell.style.bold      { style = style.add_modifier(Modifier::BOLD);       }
                if cell.style.italic    { style = style.add_modifier(Modifier::ITALIC);     }
                if cell.style.underline { style = style.add_modifier(Modifier::UNDERLINED); }
                spans.push(Span::styled(cell.ch.to_string(), style));
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn key_to_string(key: crossterm::event::KeyEvent) -> String {
    use crate::kernel::input::keys::apply_modifiers;

    let ctrl  = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt   = key.modifiers.contains(KeyModifiers::ALT);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Char(c) => apply_modifiers(c, ctrl, alt, shift),
        KeyCode::Enter     => "return".to_string(),
        KeyCode::Tab       => if shift { "shift-tab".to_string() } else { "tab".to_string() },
        KeyCode::BackTab   => "shift-tab".to_string(),
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Esc       => "esc".to_string(),
        KeyCode::Left      => "left".to_string(),
        KeyCode::Right     => "right".to_string(),
        KeyCode::Up        => "up".to_string(),
        KeyCode::Down      => "down".to_string(),
        KeyCode::Home      => "home".to_string(),
        KeyCode::End       => "end".to_string(),
        KeyCode::PageUp    => "page-up".to_string(),
        KeyCode::PageDown  => "page-down".to_string(),
        KeyCode::Delete    => "delete".to_string(),
        KeyCode::F(n)      => format!("f{}", n),
        _                  => "unknown".to_string(),
    }
}
