//! Shared key-normalisation helpers used by both the TUI and GUI adapters.
//!
//! Moving this logic to one place ensures that `shift+9` produces `"("` on
//! both backends — the class of bug that existed in the GUI adapter before
//! this module was introduced.

/// Apply the US-QWERTY shift mapping to a base character.
///
/// Terminals that report the *base* key code with a SHIFT modifier (e.g.
/// kitty protocol, extended keyboard reporting) need this transformation.
/// Terminals that already report the *produced* character (e.g. `'('` for
/// shift+9) will find the mapping is a no-op because `'('` is not in the
/// unshifted-key column.
pub fn shift_char(c: char) -> char {
    match c {
        '`'  => '~',  '1' => '!', '2' => '@', '3' => '#',
        '4'  => '$',  '5' => '%', '6' => '^', '7' => '&',
        '8'  => '*',  '9' => '(', '0' => ')',
        '-'  => '_',  '=' => '+',
        '['  => '{',  ']' => '}', '\\' => '|',
        ';'  => ':',  '\'' => '"',
        ','  => '<',  '.' => '>', '/' => '?',
        c if c.is_ascii_lowercase() => c.to_ascii_uppercase(),
        _ => c,
    }
}

/// Produce the canonical key string for a printable character and its
/// modifier state.
///
/// This is the single authority on key-string format; both the TUI and GUI
/// adapters must call this function instead of hand-rolling format strings.
pub fn apply_modifiers(base: char, ctrl: bool, alt: bool, shift: bool) -> String {
    if ctrl  { return format!("ctrl-{}", base.to_ascii_lowercase()); }
    if alt   { return format!("meta-{}", base); }
    if shift { return shift_char(base).to_string(); }
    base.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_digits_map_correctly() {
        assert_eq!(shift_char('1'), '!');
        assert_eq!(shift_char('9'), '(');
        assert_eq!(shift_char('0'), ')');
        assert_eq!(shift_char('2'), '@');
    }

    #[test]
    fn shift_uppercase_letters_are_noop() {
        assert_eq!(shift_char('A'), 'A');
        assert_eq!(shift_char('Z'), 'Z');
    }

    #[test]
    fn shift_lowercase_letters_uppercase() {
        assert_eq!(shift_char('a'), 'A');
        assert_eq!(shift_char('z'), 'Z');
    }

    #[test]
    fn apply_ctrl_lowercases_base() {
        assert_eq!(apply_modifiers('A', true, false, false), "ctrl-a");
        assert_eq!(apply_modifiers('s', true, false, false), "ctrl-s");
    }

    #[test]
    fn apply_alt() {
        assert_eq!(apply_modifiers('x', false, true, false), "meta-x");
    }

    #[test]
    fn apply_shift_digit() {
        assert_eq!(apply_modifiers('9', false, false, true), "(");
    }

    #[test]
    fn no_modifier_passthrough() {
        assert_eq!(apply_modifiers('a', false, false, false), "a");
        assert_eq!(apply_modifiers('Z', false, false, false), "Z");
    }
}
