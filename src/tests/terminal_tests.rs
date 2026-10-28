//! Terminal emulator integration tests — raw PTY-shaped byte streams land
//! on the grid the dock renders.  Parser-level sequences are covered in
//! `kernel::terminal::ansi`; these test the whole screen surface.

use crate::kernel::terminal::screen::TerminalScreen;

fn screen() -> TerminalScreen {
    TerminalScreen::with_size(20, 6)
}

fn text(s: &TerminalScreen) -> String {
    (0..s.line_count())
        .map(|i| s.line(i))
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
fn crlf_through_the_raw_pipeline_lands_on_new_lines() {
    let mut s = screen();
    s.apply("one\r\ntwo\r\nthree");
    let lines = text(&s);
    assert!(lines.starts_with("one|two|three"), "{lines}");
}

#[test]
fn progress_bar_carriage_return_overwrites_in_place() {
    let mut s = screen();
    s.apply("downloading 10%\rdownloading 50%");
    assert_eq!(s.line(0), "downloading 50%");
}

#[test]
fn full_clear_then_redraw_represents_vim_like_entry() {
    // A TUI entering the alt screen: the boot text disappears while inside.
    let mut s = screen();
    s.apply("shell prompt\r\n");
    s.apply("\x1b[?1049h\x1b[2J\x1b[H~ VIM");
    assert_eq!(s.line(0), "~ VIM", "alt screen shows the TUI");
    assert!(!text(&s).contains("shell prompt"), "main content hidden");
    s.apply("\x1b[?1049l");
    assert!(
        text(&s).starts_with("shell prompt"),
        "leaving restores the main grid"
    );
}

#[test]
fn htop_style_bottom_bar_stays_put() {
    // htop draws a header, a body, and a bottom meter bar via absolute
    // cursor moves; all three rows must coexist on the grid.
    let mut s = TerminalScreen::with_size(20, 5);
    s.apply("\x1b[1;1Hhtop  \x1b[3;1Hcpu | | |  \x1b[5;1H[| 1.0 |||]");
    assert_eq!(s.line(0), "htop");
    assert_eq!(s.line(2), "cpu | | |");
    assert_eq!(s.line(4), "[| 1.0 |||]");
}

#[test]
fn colored_output_preserves_text_and_styles() {
    let mut s = screen();
    s.apply("\x1b[32mgreen\x1b[0m plain");
    let styled = s.styled_line(0);
    assert_eq!(styled.text, "green plain");
    assert_eq!(styled.spans.len(), 2, "color run then plain run");
}

#[test]
fn truncated_chunks_parse_across_applies() {
    // A 4096-byte PTY read can split an escape sequence in half; the
    // parser state must not leak garbage into the grid.
    let mut s = screen();
    s.apply("hello \x1b[3");
    s.apply("1mred");
    let styled = s.styled_line(0);
    assert_eq!(styled.text, "hello red");
    assert_eq!(
        styled.spans[1].style.fg,
        crate::kernel::terminal::screen::Color::Indexed(1)
    );
}

#[test]
fn output_bumps_revision_for_repaint() {
    let mut s = screen();
    let before = s.revision;
    s.apply("\x1b[1;1Hx");
    assert!(s.revision > before);
}

#[test]
fn resize_matches_pty_reflow_contract() {
    let mut s = TerminalScreen::with_size(20, 6);
    s.apply("content");
    s.resize(10, 3);
    assert!(
        s.line(0).starts_with("content"),
        "existing rows survive resize"
    );
}

#[test]
fn tail_is_what_the_dock_paints() {
    let mut s = TerminalScreen::with_size(10, 3);
    for i in 0..5 {
        s.apply(&format!("line{i}\r\n"));
    }
    let tail = s.tail(2);
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0], "line4", "the newest content line");
    assert_eq!(s.tail(3)[0], "line3");
}

// ── UTF-8 chunk-split reassembly ─────────────────────────────────────

#[test]
fn utf8_sequence_split_across_chunks_is_reassembled() {
    use crate::kernel::terminal::decode_chunk;
    // "héllo" — é is a 2-byte sequence split at the read boundary.
    let mut pending = Vec::new();
    let a = decode_chunk(&mut pending, b"h\xc3");
    let b = decode_chunk(&mut pending, b"\xa9llo");
    assert_eq!(format!("{a}{b}"), "héllo");
    assert!(pending.is_empty());
}

#[test]
fn utf8_codepoint_split_three_ways_survives() {
    use crate::kernel::terminal::decode_chunk;
    // 😀 U+1F600 is 4 bytes; split one byte per read.
    let mut pending = Vec::new();
    let a = decode_chunk(&mut pending, b"\xf0");
    let b = decode_chunk(&mut pending, b"\x9f");
    let c = decode_chunk(&mut pending, b"\x98");
    let d = decode_chunk(&mut pending, b"\x80!");
    assert_eq!(format!("{a}{b}{c}{d}"), "\u{1f600}!");
    assert!(pending.is_empty());
}

#[test]
fn invalid_bytes_become_replacement_and_decoding_resumes() {
    use crate::kernel::terminal::decode_chunk;
    let mut pending = Vec::new();
    let out = decode_chunk(&mut pending, b"ok\xff\xfemax");
    assert_eq!(out, "ok\u{fffd}\u{fffd}max");
    assert!(pending.is_empty());
}
