use super::*;

#[test]
fn a_new_tree_has_one_focused_full_size_pane() {
    let mut t = ViewTree::new();
    t.resize(100, 40);
    assert_eq!(t.len(), 1);
    let p = t.panes()[0].clone();
    assert_eq!((p.x, p.y, p.width, p.height), (0, 0, 100, 39));
    assert_eq!(t.focused_window(), Some(p.id));
    assert_eq!(t.focused_index(), 0);
}

#[test]
fn splitting_inherits_the_focused_buffer_and_tiles_the_area() {
    let mut t = ViewTree::new();
    t.resize(100, 40);
    let first = t.focused_window().unwrap();
    t.set_buffer(first, 7);

    let second = t.split_horizontal(first).expect("a new pane");

    assert_eq!(t.len(), 2);
    assert_eq!(t.buffer(second), Some(7), "the split copies the buffer");
    let total: u16 = t.panes().iter().map(|p| p.width).sum();
    assert_eq!(total, 100, "splits tile the full width");
    assert_eq!(t.layout_axis, SplitDirection::Horizontal);
}

#[test]
fn vertical_splits_tile_the_height_above_the_status_line() {
    let mut t = ViewTree::new();
    t.resize(80, 24);
    let first = t.focused_window().unwrap();
    t.split_vertical(first);

    let total: u16 = t.panes().iter().map(|p| p.height).sum();
    assert_eq!(total, 23, "one row is reserved for the status line");
    assert_eq!(t.layout_axis, SplitDirection::Vertical);
}

#[test]
fn weight_changes_are_clamped_and_redistributed() {
    let mut t = ViewTree::new();
    t.resize(100, 30);
    let first = t.focused_window().unwrap();
    let second = t.split_horizontal(first).unwrap();

    t.resize_weighted(second, 5.0);
    let w = t.pane(second).unwrap().weight;
    assert!((0.01..=1.0).contains(&w), "weights stay in range: {w}");
    assert!(t.pane(second).unwrap().width > t.pane(first).unwrap().width);
}

#[test]
fn tabs_stack_at_full_size_and_take_focus() {
    let mut t = ViewTree::new();
    t.resize(90, 30);
    let tab = t.open_tab(Some(3));

    assert_eq!(t.focused_window(), Some(tab));
    assert_eq!(t.focused_buffer(), Some(3));
    let pane = t.pane(tab).unwrap();
    assert_eq!(pane.layout, LayoutConstraint::Tab);
    assert_eq!((pane.x, pane.y, pane.width, pane.height), (0, 0, 90, 29));
}

#[test]
fn the_last_pane_is_never_closed_and_focus_slides_back() {
    let mut t = ViewTree::new();
    let first = t.focused_window().unwrap();
    let second = t.split_horizontal(first).unwrap();
    t.focus(second);

    t.close(second);
    assert_eq!(t.len(), 1);
    assert_eq!(t.focused_window(), Some(first));

    t.close(first);
    assert_eq!(t.len(), 1, "the editor always keeps one pane");
}

#[test]
fn focus_cycles_and_ignores_unknown_ids() {
    let mut t = ViewTree::new();
    let first = t.focused_window().unwrap();
    let second = t.split_horizontal(first).unwrap();

    t.focus(WindowId(999));
    assert_eq!(
        t.focused_window(),
        Some(first),
        "unknown ids do not move focus"
    );

    t.focus_next();
    assert_eq!(t.focused_window(), Some(second));
    t.focus_next();
    assert_eq!(t.focused_window(), Some(first));
}

#[test]
fn reorder_swaps_pane_slots() {
    let mut t = ViewTree::new();
    let first = t.focused_window().unwrap();
    let second = t.split_horizontal(first).unwrap();

    t.reorder(first, second);
    assert_eq!(t.panes()[0].id, second);
    assert_eq!(t.panes()[1].id, first);
}

#[test]
fn restore_reattaches_panes_and_focus() {
    let mut t = ViewTree::new();
    let mut other = ViewTree::new();
    let a = other.focused_window().unwrap();
    let b = other.split_horizontal(a).unwrap();

    t.restore(other.panes().to_vec(), 1);
    assert_eq!(t.len(), 2);
    assert_eq!(t.focused_window(), Some(b));
}
