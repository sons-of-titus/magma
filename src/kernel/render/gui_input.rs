//! GUI input translation — egui events → Magma key strings + raw mouse events.
//!
//! Key strings use the same grammar as the TUI backend (`ctrl-x`, `meta-x`,
//! `ctrl-shift-x`, plain characters, and named specials like `return` / `up`).
//! The GUI intercepts a handful of chords (dialog shortcuts) before these
//! strings reach the editor's keymap.

use eframe::egui;
use crate::kernel::input::NON_INSERTABLE;
use crate::kernel::input::event::{MouseButton, MouseEventKind};

/// A mouse event in pixel coordinates, converted to cell coords
/// inside the CentralPanel closure.
pub struct RawMouseEvent {
    pub px: f32,
    pub py: f32,
    pub kind: MouseEventKind,
    pub button: MouseButton,
    pub modifiers: String,
}

/// Collect one frame of egui input into (key strings, raw mouse events).
///
/// `line_h` converts point-unit wheel deltas into scroll lines.
pub fn collect(ctx: &egui::Context, line_h: f32) -> (Vec<String>, Vec<RawMouseEvent>) {
    let mut keys = Vec::new();
    let mut mice = Vec::new();
    ctx.input(|i| {
        for ev in &i.events {
            if let Some(s) = translate_event(ev) {
                keys.push(s);
            }
            match ev {
                egui::Event::PointerButton { pos, button, pressed, modifiers } => {
                    let mb = match button {
                        egui::PointerButton::Primary   => MouseButton::Left,
                        egui::PointerButton::Secondary => MouseButton::Right,
                        egui::PointerButton::Middle    => MouseButton::Middle,
                        _                              => MouseButton::Left,
                    };
                    let mods = format_modifiers(modifiers);
                    let kind = if *pressed { MouseEventKind::Click } else { MouseEventKind::Release };
                    mice.push(RawMouseEvent { px: pos.x, py: pos.y, kind, button: mb, modifiers: mods });
                }
                egui::Event::PointerMoved(pos) if i.pointer.any_down() => {
                    mice.push(RawMouseEvent {
                        px: pos.x, py: pos.y,
                        kind: MouseEventKind::Drag,
                        button: MouseButton::Left,
                        modifiers: String::new(),
                    });
                }
                egui::Event::MouseWheel { unit, delta, modifiers } => {
                    let dy = match unit {
                        egui::MouseWheelUnit::Point => (delta.y / line_h).round() as i32,
                        egui::MouseWheelUnit::Line  => delta.y.round() as i32,
                        egui::MouseWheelUnit::Page  => delta.y.round() as i32,
                    };
                    if dy != 0 {
                        let mods = format_modifiers(modifiers);
                        mice.push(RawMouseEvent {
                            px: 0.0, py: 0.0,
                            kind: MouseEventKind::Scroll(dy),
                            button: MouseButton::Left,
                            modifiers: mods,
                        });
                    }
                }
                _ => {}
            }
        }
    });
    (keys, mice)
}

fn format_modifiers(mods: &egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if mods.ctrl || mods.command { parts.push("ctrl"); }
    if mods.shift { parts.push("shift"); }
    if mods.alt   { parts.push("alt"); }
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
                    return Some(format!("{}-\\", modifier_prefix(ctrl, shift)));
                }
                if *key == Key::Comma {
                    return Some(format!("{}-comma", modifier_prefix(ctrl, shift)));
                }
                let ch = key_char(*key)?;
                return Some(format!("{}-{}", modifier_prefix(ctrl, shift), ch));
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

/// Build the modifier prefix: `ctrl`, `ctrl-shift`, or `meta`.
/// Shift is part of the prefix so chords like `ctrl-shift-f` and `ctrl-f`
/// are distinguishable.
fn modifier_prefix(ctrl: bool, shift: bool) -> &'static str {
    match (ctrl, shift) {
        (true, true)  => "ctrl-shift",
        (true, false) => "ctrl",
        (false, _)    => "meta",
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
