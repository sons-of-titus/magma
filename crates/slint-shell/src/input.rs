//! Slint key events → Magma key strings.
//!
//! The grammar is shared with the egui frontend: modifier prefixes
//! (`ctrl`, `ctrl-shift`, `meta`), plain characters, and named keys
//! (`return`, `page-up`, `shift-left`).  Slint encodes named keys as
//! private-use code points, so bare modifiers and unmapped named keys are
//! filtered instead of being inserted as text.

use slint::platform::Key;

pub fn translate_key(text: &str, ctrl: bool, shift: bool, alt: bool, meta: bool) -> Option<String> {
    let ctrl = ctrl || meta;
    let c = text.chars().next()?;

    let named: Option<&str> = match c {
        c if c == char::from(Key::Escape) => Some("esc"),
        c if c == char::from(Key::Backspace) => Some("backspace"),
        c if c == char::from(Key::Return) => Some("return"),
        c if c == char::from(Key::Tab) => Some("tab"),
        c if c == char::from(Key::Delete) => Some("delete"),
        c if c == char::from(Key::LeftArrow) => Some("left"),
        c if c == char::from(Key::RightArrow) => Some("right"),
        c if c == char::from(Key::UpArrow) => Some("up"),
        c if c == char::from(Key::DownArrow) => Some("down"),
        c if c == char::from(Key::Home) => Some("home"),
        c if c == char::from(Key::End) => Some("end"),
        c if c == char::from(Key::PageUp) => Some("page-up"),
        c if c == char::from(Key::PageDown) => Some("page-down"),
        c if c == char::from(Key::F1) => Some("f1"),
        c if c == char::from(Key::F2) => Some("f2"),
        c if c == char::from(Key::F3) => Some("f3"),
        c if c == char::from(Key::F4) => Some("f4"),
        c if c == char::from(Key::F5) => Some("f5"),
        c if c == char::from(Key::F6) => Some("f6"),
        c if c == char::from(Key::F7) => Some("f7"),
        c if c == char::from(Key::F8) => Some("f8"),
        c if c == char::from(Key::F9) => Some("f9"),
        c if c == char::from(Key::F10) => Some("f10"),
        c if c == char::from(Key::F11) => Some("f11"),
        c if c == char::from(Key::F12) => Some("f12"),
        _ => None,
    };

    if let Some(name) = named {
        // Chords on named keys follow the same grammar as letters:
        // ctrl-return, ctrl-shift-page-up, meta-f8.  Bare shift keeps the
        // shift- prefix so selection bindings resolve.
        if ctrl || alt {
            return Some(format!("{}-{name}", modifier_prefix(ctrl, shift)));
        }
        if shift {
            return Some(format!("shift-{name}"));
        }
        return Some(name.to_string());
    }

    if is_modifier(c) {
        return None;
    }

    if ctrl || alt {
        if c == '\\' {
            return Some(format!("{}-\\", modifier_prefix(ctrl, shift)));
        }
        if c == ',' {
            return Some(format!("{}-comma", modifier_prefix(ctrl, shift)));
        }
        if c == ' ' {
            return Some(format!("{}-space", modifier_prefix(ctrl, shift)));
        }
        let ch = c.to_ascii_lowercase();
        return Some(format!("{}-{ch}", modifier_prefix(ctrl, shift)));
    }

    if c.is_control() || is_private_use(c) {
        return None;
    }
    Some(text.to_string())
}

fn is_modifier(c: char) -> bool {
    matches!(c,
        c if c == char::from(Key::Shift)
            || c == char::from(Key::Control)
            || c == char::from(Key::Alt)
            || c == char::from(Key::AltGr)
            || c == char::from(Key::Meta)
            || c == char::from(Key::ShiftR)
            || c == char::from(Key::ControlR)
            || c == char::from(Key::MetaR)
            || c == char::from(Key::CapsLock))
}

fn is_private_use(c: char) -> bool {
    matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD)
}

fn modifier_prefix(ctrl: bool, shift: bool) -> &'static str {
    match (ctrl, shift) {
        (true, true) => "ctrl-shift",
        (true, false) => "ctrl",
        (false, _) => "meta",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(key: Key) -> String {
        char::from(key).to_string()
    }

    #[test]
    fn ctrl_chord_uses_ascii_form() {
        assert_eq!(
            translate_key("s", true, false, false, false).as_deref(),
            Some("ctrl-s")
        );
    }

    #[test]
    fn ctrl_shift_chord_keeps_shift_in_the_prefix() {
        assert_eq!(
            translate_key("f", true, true, false, false).as_deref(),
            Some("ctrl-shift-f")
        );
    }

    #[test]
    fn mac_command_and_physical_control_both_map_to_ctrl() {
        assert_eq!(
            translate_key("q", true, false, false, false).as_deref(),
            Some("ctrl-q")
        );
        assert_eq!(
            translate_key("q", false, false, false, true).as_deref(),
            Some("ctrl-q")
        );
    }

    #[test]
    fn named_keys_use_the_keymap_names() {
        assert_eq!(
            translate_key(&named(Key::Return), false, false, false, false).as_deref(),
            Some("return")
        );
        assert_eq!(
            translate_key(&named(Key::Escape), false, false, false, false).as_deref(),
            Some("esc")
        );
        assert_eq!(
            translate_key(&named(Key::PageUp), false, false, false, false).as_deref(),
            Some("page-up")
        );
    }

    #[test]
    fn shift_navigation_is_distinguishable() {
        assert_eq!(
            translate_key(&named(Key::LeftArrow), false, true, false, false).as_deref(),
            Some("shift-left")
        );
    }

    #[test]
    fn ctrl_and_alt_chords_on_named_keys_carry_the_prefix() {
        // alt+return is lsp-code-actions; ctrl+arrows and ctrl+page keys
        // must not collapse into the bare key.
        assert_eq!(
            translate_key(&named(Key::Return), false, false, true, false).as_deref(),
            Some("meta-return")
        );
        assert_eq!(
            translate_key(&named(Key::Return), true, false, false, false).as_deref(),
            Some("ctrl-return")
        );
        assert_eq!(
            translate_key(&named(Key::UpArrow), true, false, false, false).as_deref(),
            Some("ctrl-up")
        );
        assert_eq!(
            translate_key(&named(Key::PageUp), true, true, false, false).as_deref(),
            Some("ctrl-shift-page-up")
        );
        assert_eq!(
            translate_key(&named(Key::F8), false, false, true, false).as_deref(),
            Some("meta-f8")
        );
        assert_eq!(
            translate_key(&named(Key::Tab), false, true, false, false).as_deref(),
            Some("shift-tab")
        );
        assert_eq!(
            translate_key(&named(Key::Escape), false, false, false, false).as_deref(),
            Some("esc")
        );
    }

    #[test]
    fn plain_and_unicode_text_pass_through() {
        assert_eq!(
            translate_key("a", false, false, false, false).as_deref(),
            Some("a")
        );
        assert_eq!(
            translate_key("é", false, false, false, false).as_deref(),
            Some("é")
        );
    }

    #[test]
    fn a_bare_modifier_is_not_text() {
        assert_eq!(
            translate_key(&named(Key::Shift), false, true, false, false),
            None
        );
        assert_eq!(
            translate_key(&named(Key::Control), true, false, false, false),
            None
        );
    }
}
