//! Tool output → diagnostics; the parser lives in the `tool_diag` crate.
//!
//! Applying findings to buffers and the quickfix list is in this module's
//! parent (`apply_tool_diagnostics`).

pub use tool_diag::{ToolDiag, parse_tool_output};

/// The `path:line:col` (or `path:line`) prefix of a task-output line, if the
/// line carries one — GUI output panes make such lines clickable jumps into
/// the code.  The full diagnostic parse lives in `tool_diag`; this only
/// decides which output rows are links.
pub fn find_location_prefix(line: &str) -> Option<(String, i32)> {
    let trimmed = line.trim_start().trim_start_matches("-->").trim_start();
    let mut path_end = None;
    for (i, c) in trimmed.char_indices() {
        if c == ':' && i > 0 {
            let rest = &trimmed[i + 1..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                path_end = Some((i, digits.parse::<i32>().ok()?));
                break;
            }
        }
        if !(c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '.' | '\\' | ' ')) {
            break;
        }
    }
    let (end, line_no) = path_end?;
    let path = trimmed[..end].trim_end();
    if path.is_empty() || (!path.contains('.') && !path.contains('/')) {
        return None;
    }
    Some((path.to_string(), line_no))
}

#[cfg(test)]
mod location_tests {
    use super::find_location_prefix as locate;

    #[test]
    fn compiler_style_locations_parse() {
        assert_eq!(
            locate("src/main.rs:42:10: unexpected token"),
            Some(("src/main.rs".into(), 42))
        );
        assert_eq!(
            locate("   --> src/lib.rs:7:9"),
            Some(("src/lib.rs".into(), 7))
        );
        assert_eq!(
            locate("src/lib.rs:7: error: something"),
            Some(("src/lib.rs".into(), 7))
        );
    }

    #[test]
    fn prose_is_not_a_link() {
        assert_eq!(locate("Compilation finished successfully"), None);
        assert_eq!(locate("hello world"), None);
        assert_eq!(locate("error[E0308]: mismatched types"), None);
    }
}
