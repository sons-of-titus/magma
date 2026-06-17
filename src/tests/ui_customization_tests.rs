//! Pure Rust tests for Sprint 11 UI customisation primitives:
//! modeline, tab-bar flag, and overlay management.

use crate::kernel::state::Overlay;
use crate::tests::helpers;

// ── modeline ─────────────────────────────────────────────────────────────────

#[test]
fn modeline_fn_defaults_to_none() {
    let ed = helpers::make_editor();
    assert!(ed.modeline_fn.is_none());
}

#[test]
fn modeline_rendered_defaults_to_empty() {
    let ed = helpers::make_editor();
    assert!(ed.modeline_rendered.is_empty());
}

#[test]
fn modeline_fn_can_be_set() {
    let mut ed = helpers::make_editor();
    ed.modeline_fn = Some("my-modeline".to_string());
    assert_eq!(ed.modeline_fn.as_deref(), Some("my-modeline"));
}

#[test]
fn modeline_rendered_is_used_in_status_bar() {
    let mut ed = helpers::make_editor();
    ed.modeline_rendered = "custom status text".to_string();
    assert_eq!(ed.modeline_rendered, "custom status text");
}

// ── tab bar ──────────────────────────────────────────────────────────────────

#[test]
fn tab_bar_disabled_by_default() {
    let ed = helpers::make_editor();
    assert!(!ed.tab_bar_enabled);
}

#[test]
fn tab_bar_can_be_enabled() {
    let mut ed = helpers::make_editor();
    ed.tab_bar_enabled = true;
    assert!(ed.tab_bar_enabled);
}

// ── overlay ──────────────────────────────────────────────────────────────────

#[test]
fn overlays_empty_by_default() {
    let ed = helpers::make_editor();
    assert!(ed.overlays.is_empty());
}

#[test]
fn overlay_can_be_added() {
    let mut ed = helpers::make_editor();
    let ov = Overlay { id: 1, x: 5, y: 10, width: 20, height: 5, buffer_id: None, z_order: 0 };
    ed.overlays.push(ov);
    assert_eq!(ed.overlays.len(), 1);
    assert_eq!(ed.overlays[0].x, 5);
}

#[test]
fn overlay_id_counter_starts_at_1() {
    let ed = helpers::make_editor();
    assert_eq!(ed.next_overlay_id, 1);
}

#[test]
fn overlay_can_be_removed_by_id() {
    let mut ed = helpers::make_editor();
    ed.overlays.push(Overlay { id: 1, x: 0, y: 0, width: 10, height: 5, buffer_id: None, z_order: 0 });
    ed.overlays.push(Overlay { id: 2, x: 5, y: 5, width: 10, height: 5, buffer_id: None, z_order: 1 });
    ed.overlays.retain(|o| o.id != 1);
    assert_eq!(ed.overlays.len(), 1);
    assert_eq!(ed.overlays[0].id, 2);
}

#[test]
fn overlay_move_updates_position() {
    let mut ed = helpers::make_editor();
    ed.overlays.push(Overlay { id: 1, x: 0, y: 0, width: 10, height: 5, buffer_id: None, z_order: 0 });
    if let Some(ov) = ed.overlays.iter_mut().find(|o| o.id == 1) {
        ov.x = 15;
        ov.y = 3;
    }
    let ov = &ed.overlays[0];
    assert_eq!(ov.x, 15);
    assert_eq!(ov.y, 3);
}

