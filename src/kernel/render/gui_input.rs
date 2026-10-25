//! GUI input translation — egui events → Magma key strings + raw mouse events.
//!
//! Key strings use a fixed grammar (`ctrl-x`, `meta-x`, `ctrl-shift-x`, plain
//! characters, and named specials like `return` / `up`) consumed by the
//! editor's keymap.  The GUI intercepts a handful of chords (dialog
//! shortcuts) before these strings reach the editor's keymap.

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

/// IME composition input (R-15): the preedit string updates or a commit.
#[derive(Debug, Clone, PartialEq)]
pub enum ImeInput {
    /// Composition updated; an empty text clears the preedit.
    Preedit { text: String, cursor_char: Option<usize> },
    /// Composition committed — insert the text.
    Commit(String),
}

/// Collect one frame of egui input into (key strings, raw mouse events, IME
/// events).
///
/// `line_h` converts point-unit wheel deltas into scroll lines.
pub fn collect(ctx: &egui::Context, line_h: f32, wheel_remain: &mut f32, wheel_remain_x: &mut f32)
    -> (Vec<String>, Vec<RawMouseEvent>, Vec<ImeInput>)
{
    let mut keys = Vec::new();
    let mut mice = Vec::new();
    let mut ime = Vec::new();
    ctx.input(|i| {
        for ev in &i.events {
            if let Some(s) = translate_event(ev) {
                keys.push(s);
            }
            match ev {
                egui::Event::Ime(ime_ev) => match ime_ev {
                    egui::ImeEvent::Preedit { text, active_range_chars } => {
                        ime.push(ImeInput::Preedit {
                            text: text.clone(),
                            cursor_char: active_range_chars.as_ref().map(|r| r.start),
                        });
                    }
                    egui::ImeEvent::Commit(text) if !text.is_empty() => {
                        ime.push(ImeInput::Commit(text.clone()));
                    }
                    _ => {}
                },
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
                egui::Event::PointerMoved(pos) if i.pointer.primary_down() => {
                    mice.push(RawMouseEvent {
                        px: pos.x, py: pos.y,
                        kind: MouseEventKind::Drag,
                        button: MouseButton::Left,
                        modifiers: String::new(),
                    });
                }
                egui::Event::MouseWheel { unit, delta, modifiers, .. } => {
                    // Fractional wheel deltas accumulate across events: a slow
                    // two-finger scroll must not stall below one line.
                    let scale = match unit {
                        egui::MouseWheelUnit::Point => 1.0 / line_h,
                        egui::MouseWheelUnit::Line  => 1.0,
                        egui::MouseWheelUnit::Page  => 10.0,
                    };
                    // Shift + vertical wheel is the conventional horizontal pan.
                    let (raw_x, raw_y) = if modifiers.shift && delta.x == 0.0 {
                        (delta.y * scale, 0.0)
                    } else {
                        (delta.x * scale, delta.y * scale)
                    };
                    let mods = format_modifiers(modifiers);
                    let (px, py) = i.pointer.latest_pos()
                        .map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0));

                    let acc_y = raw_y + *wheel_remain;
                    let dy = acc_y.trunc() as i32;
                    *wheel_remain = acc_y - acc_y.trunc();
                    if dy != 0 {
                        mice.push(RawMouseEvent {
                            px, py,
                            kind: MouseEventKind::Scroll(dy),
                            button: MouseButton::Left,
                            modifiers: mods.clone(),
                        });
                    }

                    let acc_x = raw_x + *wheel_remain_x;
                    let dx = acc_x.trunc() as i32;
                    *wheel_remain_x = acc_x - acc_x.trunc();
                    if dx != 0 {
                        mice.push(RawMouseEvent {
                            px, py,
                            kind: MouseEventKind::ScrollX(dx),
                            button: MouseButton::Left,
                            modifiers: mods,
                        });
                    }
                }
                _ => {}
            }
        }
    });
    (keys, mice, ime)
}

fn format_modifiers(mods: &egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if mods.ctrl || mods.command { parts.push("ctrl"); }
    if mods.shift { parts.push("shift"); }
    if mods.alt   { parts.push("alt"); }
    parts.join(",")
}

pub(crate) fn translate_event(ev: &egui::Event) -> Option<String> {
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
            if let Some(s) = special {
                // Left/right arrows take a ctrl/alt prefix so horizontal-scroll
                // chords (e.g. ctrl-shift-left) are addressable.  The other
                // named keys keep their current behavior.
                if (ctrl || alt) && matches!(s, "left" | "right") {
                    return Some(format!("{}-{}", modifier_prefix(ctrl, shift), s));
                }
                // Shift extends navigation (select-left, shift-tab); other
                // modifiers keep the plain name so chords stay predictable.
                if shift && !ctrl && !alt {
                    return Some(format!("shift-{s}"));
                }
                return Some(s.to_string());
            }

            if ctrl || alt {
                if *key == Key::Backslash {
                    return Some(format!("{}-\\", modifier_prefix(ctrl, shift)));
                }
                if *key == Key::Comma {
                    return Some(format!("{}-comma", modifier_prefix(ctrl, shift)));
                }
                if *key == Key::Space {
                    return Some(format!("{}-space", modifier_prefix(ctrl, shift)));
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
