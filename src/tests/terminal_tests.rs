use std::sync::{Arc, Mutex};

use crate::state::Editor;
use crate::terminal::{is_terminal, send_input, strip_ansi, TerminalSession};

// ── strip_ansi: basic ──────────────────────────────────────────────────────

#[test]
fn strip_ansi_plain_text_unchanged() {
    assert_eq!(strip_ansi("hello world"), "hello world");
}

#[test]
fn strip_ansi_color_codes_removed() {
    let input = "\x1b[31mred\x1b[0m";
    assert_eq!(strip_ansi(input), "red");
}

#[test]
fn strip_ansi_erase_display_removed() {
    let input = "\x1b[2J\x1b[Hhello";
    assert_eq!(strip_ansi(input), "hello");
}

#[test]
fn strip_ansi_osc_title_removed() {
    let input = "\x1b]0;my title\x07text";
    assert_eq!(strip_ansi(input), "text");
}

#[test]
fn strip_ansi_crlf_normalized() {
    assert_eq!(strip_ansi("line1\r\nline2"), "line1\nline2");
}

#[test]
fn strip_ansi_newline_kept() {
    assert_eq!(strip_ansi("hello\nworld"), "hello\nworld");
}

#[test]
fn strip_ansi_tab_kept() {
    assert_eq!(strip_ansi("hello\tworld"), "hello\tworld");
}

#[test]
fn strip_ansi_empty_string() {
    assert_eq!(strip_ansi(""), "");
}

#[test]
fn strip_ansi_control_chars_stripped() {
    // ^C, ^D, ^G (BEL) should be stripped
    assert_eq!(strip_ansi("ab\x03cd\x04ef"), "abcdef");
}

// ── strip_ansi: faint / ghost-text suppression ─────────────────────────────

#[test]
fn strip_ansi_faint_suppressed() {
    // Text between \x1b[2m and \x1b[m (dim) should be suppressed
    let input = "real\x1b[2mghost\x1b[mvisible";
    assert_eq!(strip_ansi(input), "realvisible");
}

#[test]
fn strip_ansi_faint_reset_with_0() {
    let input = "a\x1b[2mghost\x1b[0mvisible";
    assert_eq!(strip_ansi(input), "avisible");
}

#[test]
fn strip_ansi_faint_reset_with_22() {
    let input = "a\x1b[2mghost\x1b[22mvisible";
    assert_eq!(strip_ansi(input), "avisible");
}

#[test]
fn strip_ansi_multiple_faint_regions() {
    // \x1b[m resets faint flag, so text between regions IS emitted
    let input = "a\x1b[2mxxx\x1b[my\x1b[2mzzz\x1b[m";
    // a -> out="a"
    // \x1b[2m -> faint=true; xxx -> suppressed
    // \x1b[m -> faint=false; y -> out="ay"
    // \x1b[2m -> faint=true; zzz -> suppressed
    // \x1b[m -> faint=false
    assert_eq!(strip_ansi(input), "ay");
}

#[test]
fn strip_ansi_real_char_after_faint_resets_counter() {
    // A real (non-faint, non-control) character resets faint_suppressed to 0.
    // So subsequent \x08 after real text should produce \x08 in output.
    let input = "\x1b[2mghost\x1b[mreal\x08";
    assert_eq!(strip_ansi(input), "real\x08");
}

#[test]
fn strip_ansi_x08_eaten_by_faint_suppressed() {
    // \x08 after faint chars (with no real chars in between) should decrement
    // faint_suppressed instead of producing output.
    let input = "\x1b[2mab\x08c\x1b[m";
    // a -> faint_suppressed=1, b -> faint_suppressed=2
    // \x08 -> faint_suppressed=1, no output
    // c -> faint_suppressed=2
    // all suppressed by faint
    assert_eq!(strip_ansi(input), "");
}

// ── strip_ansi: CSI cursor-back ────────────────────────────────────────────

#[test]
fn strip_ansi_csi_cursor_back_produces_x08() {
    let input = "\x1b[3D";
    assert_eq!(strip_ansi(input), "\x08\x08\x08");
}

#[test]
fn strip_ansi_csi_cursor_back_with_faint_interaction() {
    // Faint chars are suppressed, so cursor-back within the faint region
    // should not produce \x08 for those chars.
    let input = "\x1b[2mabc\x1b[2D\x1b[mreal";
    // faint=on, a,b,c -> faint_suppressed=3
    // CSI 2D: n=2, effective = 2 - 3 = 0, faint_suppressed = 3 - 2 = 1
    // faint=off (via \x1b[m which sets faint=false but doesn't reset counter)
    // actually \x1b[m sets faint=false, but faint_suppressed stays at 1
    // 'r' is real -> faint_suppressed=0, out.push('r')
    // 'e','a','l' -> out="real"
    assert_eq!(strip_ansi(input), "real");
}

#[test]
fn strip_ansi_csi_cursor_back_exceeds_faint_suppressed() {
    let input = "\x1b[2ma\x1b[5D\x1b[mreal";
    // a -> faint_suppressed=1
    // CSI 5D: n=5, effective = 5 - 1 = 4, faint_suppressed = 0
    // out = "\x08\x08\x08\x08"
    // \x1b[m -> faint=false
    // real -> out = "\x08\x08\x08\x08real"
    assert_eq!(strip_ansi(input), "\x08\x08\x08\x08real");
}

#[test]
fn strip_ansi_csi_cursor_back_no_args_defaults_one() {
    let input = "ab\x1b[D";
    // CSI D with no params: n defaults to 1
    assert_eq!(strip_ansi(input), "ab\x08");
}

// ── strip_ansi: CR handling ────────────────────────────────────────────────

#[test]
fn strip_ansi_cr_without_lf_preserved() {
    assert_eq!(strip_ansi("line1\rline2"), "line1\rline2");
}

#[test]
fn strip_ansi_cr_resets_faint_suppressed() {
    // \r resets faint_suppressed (but not faint flag).
    // Need \x1b[m after \r to also turn off the dim attribute.
    let input = "\x1b[2mabc\r\x1b[mreal";
    // a,b,c -> faint_suppressed=3
    // \r -> faint_suppressed=0, out="\r"
    // \x1b[m -> faint=false
    // real -> out = "\rreal"
    assert_eq!(strip_ansi(input), "\rreal");
}

// ── strip_ansi: OSC ────────────────────────────────────────────────────────

#[test]
fn strip_ansi_osc_terminated_by_st() {
    // OSC can also be terminated by \x1b\\ (ST) instead of \x07
    let input = "\x1b]0;title\x1b\\text";
    assert_eq!(strip_ansi(input), "text");
}

#[test]
fn strip_ansi_osc_empty() {
    let input = "\x1b]\x07text";
    assert_eq!(strip_ansi(input), "text");
}

// ── strip_ansi: incomplete / truncated sequences ───────────────────────────

#[test]
fn strip_ansi_trailing_esc_silent() {
    assert_eq!(strip_ansi("hello\x1b"), "hello");
}

#[test]
fn strip_ansi_trailing_csi_silent() {
    assert_eq!(strip_ansi("hello\x1b["), "hello");
}

#[test]
fn strip_ansi_trailing_csi_partial_params_silent() {
    assert_eq!(strip_ansi("hello\x1b[31"), "hello");
}

#[test]
fn strip_ansi_trailing_osc_open_silent() {
    assert_eq!(strip_ansi("hello\x1b]"), "hello");
}

#[test]
fn strip_ansi_trailing_esc_paren_silent() {
    assert_eq!(strip_ansi("hello\x1b("), "hello");
}

// ── strip_ansi: special CSI sequences ──────────────────────────────────────

#[test]
fn strip_ansi_private_mode_csi_stripped() {
    // ESC[?25h (show cursor) and similar private sequences
    assert_eq!(strip_ansi("\x1b[?25h"), "");
}

#[test]
fn strip_ansi_csi_with_tilde_stripped() {
    // ESC[3~ (Delete key) and similar
    assert_eq!(strip_ansi("\x1b[3~"), "");
}

#[test]
fn strip_ansi_csi_with_semicolons_stripped() {
    // ESC[38;5;196m (256-color)
    assert_eq!(strip_ansi("\x1b[38;5;196mcolor\x1b[0m"), "color");
}

#[test]
fn strip_ansi_multiple_sequences() {
    let input = "\x1b[31m\x1b[1m\x1b[4mboldred\x1b[0m";
    assert_eq!(strip_ansi(input), "boldred");
}

// ── strip_ansi: character-set selection ────────────────────────────────────

#[test]
fn strip_ansi_esc_paren_selection_stripped() {
    // ESC ( B selects character set — should be stripped
    assert_eq!(strip_ansi("\x1b(Bhello"), "hello");
}

#[test]
fn strip_ansi_esc_o_prefix_stripped() {
    // ESC O is SS3 (single shift 3) — should be stripped along with next char
    // Actually looking at the code: Some('O') | Some('N') => { chars.next(); }
    // It consumes the ESC letter AND the next character
    assert_eq!(strip_ansi("\x1bOPhello"), "hello");
}

#[test]
fn strip_ansi_esc_n_prefix_stripped() {
    // ESC N is SS2 — stripped along with next char
    assert_eq!(strip_ansi("\x1bN$"), "");
}

// ── is_terminal ────────────────────────────────────────────────────────────

#[test]
fn is_terminal_unknown_buf_returns_false() {
    assert!(!is_terminal(
        &Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new())),
        999
    ));
}

#[test]
fn is_terminal_present_returns_true() {
    let mut ed = Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    ed.io.terminals.insert(42, session);
    assert!(is_terminal(&ed, 42));
}

#[test]
fn is_terminal_after_removal_returns_false() {
    let mut ed = Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    ed.io.terminals.insert(42, session);
    ed.io.terminals.remove(&42);
    assert!(!is_terminal(&ed, 42));
}

#[test]
fn is_terminal_multiple_buffers() {
    let mut ed = Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    let make = || TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    ed.io.terminals.insert(1, make());
    ed.io.terminals.insert(2, make());
    assert!(is_terminal(&ed, 1));
    assert!(is_terminal(&ed, 2));
    assert!(!is_terminal(&ed, 3));
}

// ── send_input ─────────────────────────────────────────────────────────────

#[test]
fn send_input_unknown_buf_errors() {
    let ed = Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    let r = send_input(&ed, 999, "hello");
    assert!(r.is_err());
    assert!(r.unwrap_err().contains("not a terminal"));
}

#[test]
fn send_input_known_buf_succeeds() {
    let mut ed = Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    ed.io.terminals.insert(7, session);
    let r = send_input(&ed, 7, "hello");
    assert!(r.is_ok());
}

#[test]
fn send_input_multiple_writes() {
    let mut ed = Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    ed.io.terminals.insert(7, session);
    assert!(send_input(&ed, 7, "a").is_ok());
    assert!(send_input(&ed, 7, "b").is_ok());
    assert!(send_input(&ed, 7, "c").is_ok());
}

#[test]
fn send_input_empty_string_succeeds() {
    let mut ed = Editor::new(Box::new(crate::fs::disk::DiskFileSystem::new()));
    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    ed.io.terminals.insert(7, session);
    let r = send_input(&ed, 7, "");
    assert!(r.is_ok());
}

// ── TerminalSession ────────────────────────────────────────────────────────

#[test]
fn terminal_session_construct() {
    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    // The stdin field should be writable via the mutex
    use std::io::Write;
    let mut writer = session.stdin.lock().unwrap();
    let n = writer.write(b"test").unwrap();
    assert_eq!(n, 4);
}

#[test]
fn terminal_session_multiple_writers_same_session() {
    let session = TerminalSession {
        stdin: Arc::new(Mutex::new(Box::new(std::io::sink()))),
    };
    {
        let _w1 = session.stdin.lock().unwrap();
        // Would deadlock if we tried to lock again — Arc allows shared ownership
        // but Mutex enforces single access. Test that the Arc exists.
    }
    // After dropping the guard, we can lock again
    let _w2 = session.stdin.lock().unwrap();
}
