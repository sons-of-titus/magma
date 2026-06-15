//! Pure Rust tests for Sprint 11 UI customisation primitives:
//! modeline, tab-bar flag, overlay management, and gutter signs.

use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::state::{Editor, GutterSign, Overlay};

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    ed
}

// ── modeline ─────────────────────────────────────────────────────────────────

#[test]
fn modeline_fn_defaults_to_none() {
    let ed = make_editor();
    assert!(ed.modeline_fn.is_none());
}

#[test]
fn modeline_rendered_defaults_to_empty() {
    let ed = make_editor();
    assert!(ed.modeline_rendered.is_empty());
}

#[test]
fn modeline_fn_can_be_set() {
    let mut ed = make_editor();
    ed.modeline_fn = Some("my-modeline".to_string());
    assert_eq!(ed.modeline_fn.as_deref(), Some("my-modeline"));
}

#[test]
fn modeline_rendered_is_used_in_status_bar() {
    let mut ed = make_editor();
    ed.modeline_rendered = "custom status text".to_string();
    assert_eq!(ed.modeline_rendered, "custom status text");
}

// ── tab bar ──────────────────────────────────────────────────────────────────

#[test]
fn tab_bar_disabled_by_default() {
    let ed = make_editor();
    assert!(!ed.tab_bar_enabled);
}

#[test]
fn tab_bar_can_be_enabled() {
    let mut ed = make_editor();
    ed.tab_bar_enabled = true;
    assert!(ed.tab_bar_enabled);
}

// ── overlay ──────────────────────────────────────────────────────────────────

#[test]
fn overlays_empty_by_default() {
    let ed = make_editor();
    assert!(ed.overlays.is_empty());
}

#[test]
fn overlay_can_be_added() {
    let mut ed = make_editor();
    let ov = Overlay { id: 1, x: 5, y: 10, width: 20, height: 5, buffer_id: None, z_order: 0 };
    ed.overlays.push(ov);
    assert_eq!(ed.overlays.len(), 1);
    assert_eq!(ed.overlays[0].x, 5);
}

#[test]
fn overlay_id_counter_starts_at_1() {
    let ed = make_editor();
    assert_eq!(ed.next_overlay_id, 1);
}

#[test]
fn overlay_can_be_removed_by_id() {
    let mut ed = make_editor();
    ed.overlays.push(Overlay { id: 1, x: 0, y: 0, width: 10, height: 5, buffer_id: None, z_order: 0 });
    ed.overlays.push(Overlay { id: 2, x: 5, y: 5, width: 10, height: 5, buffer_id: None, z_order: 1 });
    ed.overlays.retain(|o| o.id != 1);
    assert_eq!(ed.overlays.len(), 1);
    assert_eq!(ed.overlays[0].id, 2);
}

#[test]
fn overlay_move_updates_position() {
    let mut ed = make_editor();
    ed.overlays.push(Overlay { id: 1, x: 0, y: 0, width: 10, height: 5, buffer_id: None, z_order: 0 });
    if let Some(ov) = ed.overlays.iter_mut().find(|o| o.id == 1) {
        ov.x = 15;
        ov.y = 3;
    }
    let ov = &ed.overlays[0];
    assert_eq!(ov.x, 15);
    assert_eq!(ov.y, 3);
}

// ── gutter signs ─────────────────────────────────────────────────────────────

#[test]
fn gutter_signs_empty_by_default() {
    let ed = make_editor();
    assert!(ed.gutter.signs.is_empty());
}

#[test]
fn gutter_sign_can_be_set_for_buffer_line() {
    let mut ed = make_editor();
    let sign = GutterSign { face: "error-face".to_string(), text: "E".to_string(), priority: 10 };
    ed.gutter.signs.entry(0).or_default().entry(5).or_default().push(sign);
    let signs = ed.gutter.signs.get(&0).unwrap().get(&5).unwrap();
    assert_eq!(signs.len(), 1);
    assert_eq!(signs[0].text, "E");
}

#[test]
fn gutter_signs_cleared_for_buffer() {
    let mut ed = make_editor();
    ed.gutter.signs.entry(0).or_default().entry(3).or_default().push(
        GutterSign { face: "warning-face".to_string(), text: "W".to_string(), priority: 5 }
    );
    ed.gutter.signs.remove(&0);
    assert!(!ed.gutter.signs.contains_key(&0));
}

#[test]
fn gutter_sign_highest_priority_picked() {
    let mut ed = make_editor();
    let low  = GutterSign { face: "warning-face".to_string(), text: "W".to_string(), priority: 5 };
    let high = GutterSign { face: "error-face".to_string(), text: "E".to_string(), priority: 10 };
    let line_signs = ed.gutter.signs.entry(0).or_default().entry(0).or_default();
    line_signs.push(low);
    line_signs.push(high);
    let top = ed.gutter.signs[&0][&0].iter().max_by_key(|s| s.priority).unwrap();
    assert_eq!(top.text, "E");
}
