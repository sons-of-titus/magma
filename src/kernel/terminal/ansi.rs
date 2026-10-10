//! ANSI/xterm escape-sequence parser — feeds `TerminalScreen`.
//!
//! A char-level state machine: Ground → Esc → CSI (params) / OSC.  The
//! sequences a real shell or TUI emits are dispatched onto the grid;
//! unknown finals are ignored, never fatal.

use super::screen::{Cell, Color, TerminalScreen};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Ground,
    Esc,
    Csi,
    Osc,
    Charset,
}

pub(super) fn feed(screen: &mut TerminalScreen, data: &str) {
    if data.is_empty() { return; }
    for ch in data.chars() {
        match screen.parser.phase {
            Phase::Ground => match ch {
                '\x1b' => screen.parser.phase = Phase::Esc,
                '\r' => {
                    screen.cursor_col = 0;
                    screen.pending_wrap = false;
                }
                '\n' | '\x0b' | '\x0c' => screen.index(),
                '\x08' => {
                    screen.pending_wrap = false;
                    screen.cursor_col = screen.cursor_col.saturating_sub(1);
                }
                '\t' => {
                    screen.pending_wrap = false;
                    let col = screen.cursor_col;
                    screen.cursor_col = ((col / 8) + 1) * 8;
                    if screen.cursor_col >= screen.cols {
                        screen.cursor_col = screen.cols - 1;
                    }
                }
                '\x07' => {}
                c if (c as u32) < 32 || c == '\x7f' => {}
                c => print_char(screen, c),
            },
            Phase::Esc => {
                screen.parser.phase = match ch {
                    '[' => {
                        screen.parser.private = false;
                        screen.parser.params.clear();
                        screen.parser.intermediates.clear();
                        Phase::Csi
                    }
                    ']' => Phase::Osc,
                    '(' | ')' => Phase::Charset,
                    '7' => {
                        screen.saved_cursor = (screen.cursor_row, screen.cursor_col);
                        Phase::Ground
                    }
                    '8' => {
                        (screen.cursor_row, screen.cursor_col) = screen.saved_cursor;
                        screen.pending_wrap = false;
                        Phase::Ground
                    }
                    'D' => {
                        screen.index();
                        Phase::Ground
                    }
                    'M' => {
                        screen.reverse_index();
                        Phase::Ground
                    }
                    'E' => {
                        screen.cursor_col = 0;
                        screen.index();
                        Phase::Ground
                    }
                    'c' => {
                        full_reset(screen);
                        Phase::Ground
                    }
                    _ => Phase::Ground,
                };
            }
            Phase::Charset => {
                // One charset designator byte, then back to ground.
                screen.parser.phase = Phase::Ground;
            }
            Phase::Csi => match ch {
                '0'..='9' | ';' | ':' => screen.parser.params.push(ch),
                '?' | '>' | '<' | '=' if screen.parser.params.is_empty() && screen.parser.intermediates.is_empty() => {
                    screen.parser.private = ch == '?';
                }
                '\x20'..='\x2f' => screen.parser.intermediates.push(ch),
                final_byte @ '\x40'..='\x7e' => {
                    dispatch_csi(screen, final_byte);
                    screen.parser.phase = Phase::Ground;
                }
                _ => {
                    // Malformed sequence: abandon it.
                    screen.parser.phase = Phase::Ground;
                }
            },
            Phase::Osc => match ch {
                '\x07' => screen.parser.phase = Phase::Ground,
                '\x1b' => {
                    // ST is ESC \ — a lone ESC re-enters Esc state; the
                    // following '\' is an ignored Esc final.
                    screen.parser.phase = Phase::Esc;
                }
                _ => {}
            },
        }
    }
    screen.revision += 1;
}

struct CsiRef<'a> {
    params: &'a str,
}

impl CsiRef<'_> {
    fn parts(&self) -> impl Iterator<Item = Option<u16>> + '_ {
        self.params.split(|c| c == ':' || c == ';').map(|p| {
            if p.is_empty() { None } else { p.parse().ok() }
        })
    }

    fn param(&self, idx: usize, default: u16) -> u16 {
        self.parts().nth(idx).flatten().unwrap_or(default)
    }

    fn count(&self, idx: usize) -> u16 {
        self.param(idx, 1).max(1)
    }
}

fn print_char(screen: &mut TerminalScreen, ch: char) {
    if screen.pending_wrap && screen.autowrap {
        screen.cursor_col = 0;
        screen.index();
        screen.pending_wrap = false;
    }
    let cell = Cell { ch, style: screen.style };
    screen.put_cell(screen.cursor_row, screen.cursor_col, cell);
    if screen.cursor_col + 1 >= screen.cols {
        screen.pending_wrap = true;
    } else {
        screen.cursor_col += 1;
    }
}

fn full_reset(screen: &mut TerminalScreen) {
    screen.style.reset();
    screen.cursor_row = 0;
    screen.cursor_col = 0;
    screen.pending_wrap = false;
    screen.autowrap = true;
    screen.cursor_visible = true;
    screen.scroll_top = 0;
    screen.scroll_bottom = screen.rows - 1;
    screen.alt = None;
    screen.revision += 1;
}

fn dispatch_csi(screen: &mut TerminalScreen, final_byte: char) {
    let params = screen.parser.params.clone();
    let csi = CsiRef { params: &params };
    match final_byte {
        'A' => screen.cursor_row = screen.cursor_row.saturating_sub(csi.count(0)),
        'B' | 'e' => screen.cursor_row = (screen.cursor_row + csi.count(0)).min(screen.rows - 1),
        'C' | 'a' => screen.cursor_col = (screen.cursor_col + csi.count(0)).min(screen.cols - 1),
        'D' => {
            screen.pending_wrap = false;
            screen.cursor_col = screen.cursor_col.saturating_sub(csi.count(0));
        }
        'E' => {
            screen.cursor_row = (screen.cursor_row + csi.count(0)).min(screen.rows - 1);
            screen.cursor_col = 0;
        }
        'F' => {
            screen.cursor_row = screen.cursor_row.saturating_sub(csi.count(0));
            screen.cursor_col = 0;
        }
        'G' | '`' => screen.cursor_col = (csi.param(0, 1).saturating_sub(1)).min(screen.cols - 1),
        'd' => screen.cursor_row = (csi.param(0, 1).saturating_sub(1)).min(screen.rows - 1),
        'H' | 'f' => {
            screen.cursor_row = (csi.param(0, 1).saturating_sub(1)).min(screen.rows - 1);
            screen.cursor_col = (csi.param(1, 1).saturating_sub(1)).min(screen.cols - 1);
            screen.pending_wrap = false;
        }
        'J' => screen.erase_in_display(csi.param(0, 0)),
        'K' => screen.erase_in_line(csi.param(0, 0)),
        'L' => insert_lines(screen, csi.count(0)),
        'M' => delete_lines(screen, csi.count(0)),
        'P' => delete_chars(screen, csi.count(0)),
        '@' => insert_chars(screen, csi.count(0)),
        'X' => {
            // ECH: erase n chars in place, cursor stays.
            let n = csi.count(0);
            let end = (screen.cursor_col + n).min(screen.cols);
            let row = screen.cursor_row;
            let blank = Cell { ch: ' ', style: screen.style };
            for col in screen.cursor_col..end {
                screen.put_cell(row, col, blank);
            }
        }
        'S' => screen.scroll_up(csi.count(0)),
        'T' => screen.scroll_down(csi.count(0)),
        'm' => sgr(screen, &csi.params),
        'r' => {
            let top = csi.param(0, 1).saturating_sub(1);
            let bottom = csi.param(1, screen.rows).saturating_sub(1).min(screen.rows - 1);
            if top < bottom {
                screen.scroll_top = top;
                screen.scroll_bottom = bottom;
                screen.cursor_row = 0;
                screen.cursor_col = 0;
            }
        }
        's' => screen.saved_cursor = (screen.cursor_row, screen.cursor_col),
        'u' => {
            (screen.cursor_row, screen.cursor_col) = screen.saved_cursor;
            screen.pending_wrap = false;
        }
        'h' | 'l' if screen.parser.private => set_private_mode(screen, &csi, final_byte == 'h'),
        'h' | 'l' => {}
        // DSR, DA, window ops, and everything unknown: ignored.
        _ => {}
    }
}

fn set_private_mode(screen: &mut TerminalScreen, csi: &CsiRef<'_>, on: bool) {
    for mode in csi.parts().flatten() {
        match mode {
            7 => screen.autowrap = on,
            25 => screen.cursor_visible = on,
            47 | 1047 => {
                if on { screen.enter_alt(); } else { screen.leave_alt(); }
            }
            1049 => {
                if on {
                    screen.enter_alt();
                    screen.erase_in_display(2);
                    screen.cursor_row = 0;
                    screen.cursor_col = 0;
                } else {
                    screen.leave_alt();
                }
            }
            _ => {}
        }
    }
}

fn insert_lines(screen: &mut TerminalScreen, n: u16) {
    // IL moves rows from the cursor to the region's bottom down by n; the
    // opened gap is blanked.  Outside the region it does nothing.
    if screen.cursor_row < screen.scroll_top || screen.cursor_row > screen.scroll_bottom {
        return;
    }
    let top = screen.cursor_row as usize;
    let bottom = screen.scroll_bottom as usize;
    let cols = screen.cols as usize;
    let n = n.min((bottom - top + 1) as u16) as usize;
    for row in ((top + n)..=bottom).rev() {
        let src = (row - n) * cols;
        let dst = row * cols;
        screen.cells.copy_within(src..src + cols, dst);
    }
    let blank = screen.blank_with(screen.style);
    for row in top..top + n {
        let dst = row * cols;
        screen.cells[dst..dst + cols].copy_from_slice(&blank);
    }
    screen.revision += 1;
}

fn delete_lines(screen: &mut TerminalScreen, n: u16) {
    if screen.cursor_row < screen.scroll_top || screen.cursor_row > screen.scroll_bottom {
        return;
    }
    let top = screen.cursor_row as usize;
    let bottom = screen.scroll_bottom as usize;
    let cols = screen.cols as usize;
    let n = n.min((bottom - top + 1) as u16) as usize;
    for row in top..=(bottom - n) {
        let src = (row + n) * cols;
        let dst = row * cols;
        screen.cells.copy_within(src..src + cols, dst);
    }
    let blank = screen.blank_with(screen.style);
    for row in (bottom - n + 1)..=bottom {
        let dst = row * cols;
        screen.cells[dst..dst + cols].copy_from_slice(&blank);
    }
    screen.revision += 1;
}

fn insert_chars(screen: &mut TerminalScreen, n: u16) {
    let row = screen.cursor_row;
    let col = screen.cursor_col as usize;
    let cols = screen.cols as usize;
    let mut line: Vec<Cell> = ((col..cols).map(|c| screen.cell(row, c as u16))).collect();
    line.truncate(cols.saturating_sub(col).saturating_sub(n as usize));
    let blank = Cell { ch: ' ', style: screen.style };
    let mut next = vec![blank; n as usize];
    next.extend(line);
    for (i, cell) in next.into_iter().enumerate() {
        screen.put_cell(row, (col + i) as u16, cell);
    }
}

fn delete_chars(screen: &mut TerminalScreen, n: u16) {
    let row = screen.cursor_row;
    let col = screen.cursor_col as usize;
    let cols = screen.cols as usize;
    let blank = Cell { ch: ' ', style: screen.style };
    for c in col..cols {
        let src = c + n as usize;
        let cell = if src < cols { screen.cell(row, src as u16) } else { blank };
        screen.put_cell(row, c as u16, cell);
    }
}

fn sgr(screen: &mut TerminalScreen, params: &str) {
    let normalized = params.replace(':', ";");
    let codes: Vec<Option<u16>> = normalized.split(';')
        .map(|p| if p.is_empty() { None } else { p.parse().ok() })
        .collect();
    let mut i = 0;
    while i < codes.len() {
        match codes[i].unwrap_or(0) {
            0 => screen.style.reset(),
            1 => screen.style.bold = true,
            2 => screen.style.dim = true,
            3 => screen.style.italic = true,
            4 => screen.style.underline = true,
            7 => screen.style.reverse = true,
            9 => screen.style.strike = true,
            22 => { screen.style.bold = false; screen.style.dim = false; }
            23 => screen.style.italic = false,
            24 => screen.style.underline = false,
            27 => screen.style.reverse = false,
            29 => screen.style.strike = false,
            30..=37 => screen.style.fg = Color::Indexed(codes[i].unwrap() as u8 - 30),
            39 => screen.style.fg = Color::Default,
            40..=47 => screen.style.bg = Color::Indexed(codes[i].unwrap() as u8 - 40),
            49 => screen.style.bg = Color::Default,
            90..=97 => screen.style.fg = Color::Indexed(codes[i].unwrap() as u8 - 90 + 8),
            100..=107 => screen.style.bg = Color::Indexed(codes[i].unwrap() as u8 - 100 + 8),
            38 | 48 => {
                // Extended color: 38;5;n (256) or 38;2;r;g;b, possibly with
                // empty params from colon form (38;;n after normalization).
                let mut j = i + 1;
                while j < codes.len() && codes[j].is_none() { j += 1; }
                let kind = codes.get(j).copied().flatten();
                let is_fg = codes[i].unwrap() == 38;
                let mut target = |color: Color| {
                    if is_fg { screen.style.fg = color; } else { screen.style.bg = color; }
                };
                match kind {
                    Some(5) => {
                        if let Some(Some(idx)) = codes.get(j + 1).copied() {
                            target(Color::Indexed(idx as u8));
                        }
                        i = j + 2;
                        continue;
                    }
                    Some(2) => {
                        let r = codes.get(j + 1).copied().flatten().unwrap_or(0) as u8;
                        let g = codes.get(j + 2).copied().flatten().unwrap_or(0) as u8;
                        let b = codes.get(j + 3).copied().flatten().unwrap_or(0) as u8;
                        target(Color::Rgb(r, g, b));
                        i = j + 4;
                        continue;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{feed, TerminalScreen};
    use super::super::screen::Color;

    fn screen() -> TerminalScreen {
        TerminalScreen::with_size(10, 4)
    }

    fn text(s: &TerminalScreen) -> String {
        (0..s.line_count()).map(|i| s.line(i)).collect::<Vec<_>>().join("|")
    }

    #[test]
    fn plain_text_fills_the_grid() {
        let mut s = screen();
        feed(&mut s, "hello");
        assert_eq!(s.line(0), "hello");
    }

    #[test]
    fn cursor_addressing_places_text() {
        let mut s = screen();
        feed(&mut s, "\x1b[2;3Hx");
        assert_eq!(s.line(1), "  x");
    }

    #[test]
    fn line_wrap_at_the_right_margin() {
        let mut s = TerminalScreen::with_size(5, 3);
        feed(&mut s, "abcdefgh");
        assert_eq!(s.line(0), "abcde");
        assert_eq!(s.line(1), "fgh");
    }

    #[test]
    fn backspace_and_overwrite() {
        let mut s = screen();
        feed(&mut s, "abc\x08\x08X");
        assert_eq!(s.line(0), "aXc");
    }

    #[test]
    fn sgr_sets_styles_and_reset_clears() {
        let mut s = screen();
        feed(&mut s, "\x1b[1;31mR\x1b[0mg");
        let styled = s.styled_line(0);
        assert_eq!(styled.text, "Rg");
        assert_eq!(styled.spans.len(), 2);
        assert!(styled.spans[0].style.bold);
        assert_eq!(styled.spans[0].style.fg, Color::Indexed(1));
        assert_eq!(styled.spans[1].style.fg, Color::Default);
    }

    #[test]
    fn sgr_256_and_rgb() {
        let mut s = screen();
        feed(&mut s, "\x1b[38;5;200mA\x1b[48;2;1;2;3mB");
        let spans = s.styled_line(0).spans;
        assert_eq!(spans[0].style.fg, Color::Indexed(200));
        assert_eq!(spans[1].style.bg, Color::Rgb(1, 2, 3));
    }

    #[test]
    fn erase_line_and_display() {
        let mut s = screen();
        feed(&mut s, "abcdef\x1b[1;1H\x1b[K");
        assert_eq!(s.line(0), "");
        let mut s = screen();
        feed(&mut s, "one\rtwo\x1b[2J");
        assert_eq!(s.line(0), "");
    }

    #[test]
    fn el_clears_from_cursor() {
        let mut s = screen();
        feed(&mut s, "abcdef\x1b[1;4H\x1b[K");
        assert_eq!(s.line(0), "abc");
    }

    #[test]
    fn alt_screen_swap_is_lossless() {
        let mut s = screen();
        feed(&mut s, "main");
        feed(&mut s, "\x1b[?1049h");
        feed(&mut s, "\x1b[2Jalt stuff");
        assert_eq!(s.line(0), "alt stuff");
        feed(&mut s, "\x1b[?1049l");
        assert_eq!(s.line(0), "main", "leaving the alt screen restores the main grid");
    }

    #[test]
    fn scroll_region_scrolls_only_inside_itself() {
        let mut s = TerminalScreen::with_size(10, 4);
        feed(&mut s, "a\r\nb\r\nc\r\nd");
        // Region = rows 1..3 (0-based).  LF at the region bottom scrolls
        // the region up one: "b" is pushed out, rows shift, row 0 untouched.
        feed(&mut s, "\x1b[2;4r\x1b[4;1H\nX");
        assert_eq!(s.line(0), "a", "outside the region nothing scrolls");
        assert_eq!(s.line(1), "c");
        assert_eq!(s.line(3), "X", "the cursor stays at the region bottom");
    }

    #[test]
    fn insert_and_delete_lines() {
        let mut s = screen();
        feed(&mut s, "a\r\nb");
        feed(&mut s, "\x1b[1;1H\x1b[1L");
        assert_eq!(s.line(0), "");
        assert_eq!(s.line(1), "a");
        feed(&mut s, "\x1b[1M");
        assert_eq!(s.line(0), "a");
    }

    #[test]
    fn scrollback_receives_scrolled_lines() {
        let mut s = TerminalScreen::with_size(5, 3);
        for i in 0..6 {
            feed(&mut s, &format!("L{i}\r\n"));
        }
        assert!(s.line_count() > 3, "scrolled rows go to scrollback");
        assert_eq!(s.line(0), "L0");
        assert_eq!(s.tail(2)[0], "L5", "the live screen's last content line");
    }

    #[test]
    fn osc_titles_are_swallowed() {
        let mut s = screen();
        feed(&mut s, "\x1b]0;window title\x07ok");
        assert_eq!(s.line(0), "ok");
    }

    #[test]
    fn tabs_advance_to_eight() {
        let mut s = screen();
        feed(&mut s, "a\tb");
        assert_eq!(s.line(0).len(), 9);
    }

    #[test]
    fn cursor_visibility_mode() {
        let mut s = screen();
        feed(&mut s, "\x1b[?25l");
        assert!(!s.cursor_visible());
        feed(&mut s, "\x1b[?25h");
        assert!(s.cursor_visible());
    }

    #[test]
    fn resize_keeps_content_and_clamps_cursor() {
        let mut s = TerminalScreen::with_size(10, 4);
        feed(&mut s, "\x1b[4;9Hx");
        s.resize(5, 2);
        assert_eq!(s.line(0), "");
        assert!(s.line_count() >= 2);
    }

    #[test]
    fn ghost_suggestion_faint_text_renders_dim_not_corrupted() {
        // zsh-autosuggestions: faint text + cursor-back.  A real emulator
        // keeps the ghost text visible (dim) on the same line.
        let mut s = screen();
        feed(&mut s, "ls\x1b[2m --all\x1b[0m\x1b[6D");
        let styled = s.styled_line(0);
        assert_eq!(styled.text, "ls --all");
        assert!(styled.spans.iter().any(|sp| sp.style.dim), "the ghost run stays dim");
        assert_eq!(s.cursor_col(), 2);
    }
}
