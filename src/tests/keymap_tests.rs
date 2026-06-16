use crate::kernel::keymap::KeymapManager;

fn make_km() -> KeymapManager {
    KeymapManager::new()
}

// ── Basic resolution ─────────────────────────────────────────────────

#[test]
fn empty_manager_resolves_nothing() {
    let km = make_km();
    assert_eq!(km.resolve("any-key"), None);
}

#[test]
fn global_binding_resolves() {
    let mut km = make_km();
    km.set("ctrl-s", "save-buffer");
    assert_eq!(km.resolve("ctrl-s"), Some("save-buffer".into()));
}

#[test]
fn unbound_key_returns_none() {
    let km = make_km();
    assert_eq!(km.resolve("h"), None);
}

#[test]
fn multiple_global_bindings_resolve_independently() {
    let mut km = make_km();
    km.set("h", "cursor-left");
    km.set("j", "cursor-down");
    km.set("k", "cursor-up");
    km.set("l", "cursor-right");
    assert_eq!(km.resolve("h"), Some("cursor-left".into()));
    assert_eq!(km.resolve("j"), Some("cursor-down".into()));
    assert_eq!(km.resolve("k"), Some("cursor-up".into()));
    assert_eq!(km.resolve("l"), Some("cursor-right".into()));
    assert_eq!(km.resolve("a"), None);
}

#[test]
fn set_overwrites_previous_global_binding() {
    let mut km = make_km();
    km.set("ctrl-s", "save-buffer");
    km.set("ctrl-s", "write-file");
    assert_eq!(km.resolve("ctrl-s"), Some("write-file".into()));
}

#[test]
fn set_then_unset_returns_to_none() {
    let mut km = make_km();
    km.set("ctrl-q", "quit");
    assert_eq!(km.resolve("ctrl-q"), Some("quit".into()));
    km.unset("ctrl-q");
    assert_eq!(km.resolve("ctrl-q"), None);
}

#[test]
fn unset_nonexistent_key_is_safe() {
    let mut km = make_km();
    km.unset("never-set"); // must not panic
    assert_eq!(km.resolve("never-set"), None);
}

// ── Layer management ─────────────────────────────────────────────────

#[test]
fn push_layer_creates_layer_if_absent() {
    let mut km = make_km();
    km.push_layer("vim");
    // layer exists and is active, even without any bindings
    assert!(km.is_layer_active("vim"));
}

#[test]
fn push_layer_is_idempotent() {
    let mut km = make_km();
    km.push_layer("vim");
    km.push_layer("vim"); // second push is a no-op
    // Idempotent means one pop is enough to deactivate
    assert!(km.is_layer_active("vim"));
    km.pop_layer("vim");
    assert!(!km.is_layer_active("vim"));
}

#[test]
fn pop_layer_on_non_active_layer_is_safe() {
    let mut km = make_km();
    km.pop_layer("insert"); // never pushed — must not panic
}

#[test]
fn is_layer_active_after_push_pop_cycle() {
    let mut km = make_km();
    assert!(!km.is_layer_active("insert"));
    km.push_layer("insert");
    assert!(km.is_layer_active("insert"));
    km.pop_layer("insert");
    assert!(!km.is_layer_active("insert"));
}

#[test]
fn many_layers_can_be_active_simultaneously() {
    let mut km = make_km();
    km.push_layer("a");
    km.push_layer("b");
    km.push_layer("c");
    km.push_layer("d");
    km.push_layer("e");
    assert!(km.is_layer_active("a"));
    assert!(km.is_layer_active("b"));
    assert!(km.is_layer_active("c"));
    assert!(km.is_layer_active("d"));
    assert!(km.is_layer_active("e"));
    // active_layers includes "global" + 5 layers
    assert_eq!(km.active_layers().len(), 6);
}

// ── Layer priority ───────────────────────────────────────────────────

#[test]
fn active_layer_overrides_global() {
    let mut km = make_km();
    km.set("h", "global-cmd");
    km.set_layer("vim", "h", "cursor-left");
    km.push_layer("vim");
    assert_eq!(km.resolve("h"), Some("cursor-left".into()));
}

#[test]
fn later_pushed_layer_has_higher_priority() {
    let mut km = make_km();
    km.set_layer("vim",    "h", "cursor-left");
    km.set_layer("insert", "h", "insert-h");
    km.push_layer("vim");
    km.push_layer("insert");
    assert_eq!(km.resolve("h"), Some("insert-h".into()));
}

#[test]
fn later_pushed_layer_does_not_override_unbound_keys() {
    let mut km = make_km();
    km.set_layer("vim",    "h", "cursor-left");
    km.set_layer("insert", "j", "insert-j");
    km.push_layer("vim");
    km.push_layer("insert");
    assert_eq!(km.resolve("h"), Some("cursor-left".into()));
    assert_eq!(km.resolve("j"), Some("insert-j".into()));
}

#[test]
fn inactive_layer_not_consulted() {
    let mut km = make_km();
    km.set_layer("vim", "h", "cursor-left");
    assert_eq!(km.resolve("h"), None);
}

#[test]
fn global_key_resolves_when_no_layer_binds_it() {
    let mut km = make_km();
    km.set("ctrl-q", "quit");
    km.set_layer("vim", "h", "cursor-left");
    km.push_layer("vim");
    assert_eq!(km.resolve("ctrl-q"), Some("quit".into()));
}

#[test]
fn layer_chain_falls_through_to_global() {
    let mut km = make_km();
    km.set("ctrl-q", "quit");
    km.set_layer("a", "h", "left");
    km.set_layer("b", "j", "down");
    km.push_layer("a");
    km.push_layer("b");
    // 'a' not in any layer → global
    assert_eq!(km.resolve("ctrl-q"), Some("quit".into()));
    // 'h' in layer a
    assert_eq!(km.resolve("h"), Some("left".into()));
    // 'j' in layer b (higher priority)
    assert_eq!(km.resolve("j"), Some("down".into()));
}

#[test]
fn deep_stack_resolution() {
    let mut km = make_km();
    km.set("key", "global");
    km.set_layer("l1", "key", "layer-1");
    km.set_layer("l2", "key", "layer-2");
    km.set_layer("l3", "key", "layer-3");
    km.set_layer("l4", "key", "layer-4");
    km.set_layer("l5", "key", "layer-5");
    km.push_layer("l1");
    assert_eq!(km.resolve("key"), Some("layer-1".into()));
    km.push_layer("l2");
    assert_eq!(km.resolve("key"), Some("layer-2".into()));
    km.push_layer("l3");
    assert_eq!(km.resolve("key"), Some("layer-3".into()));
    km.push_layer("l4");
    assert_eq!(km.resolve("key"), Some("layer-4".into()));
    km.push_layer("l5");
    assert_eq!(km.resolve("key"), Some("layer-5".into()));

    // Pop back down
    km.pop_layer("l5");
    assert_eq!(km.resolve("key"), Some("layer-4".into()));
    km.pop_layer("l4");
    assert_eq!(km.resolve("key"), Some("layer-3".into()));
    km.pop_layer("l3");
    assert_eq!(km.resolve("key"), Some("layer-2".into()));
    km.pop_layer("l2");
    assert_eq!(km.resolve("key"), Some("layer-1".into()));
    km.pop_layer("l1");
    assert_eq!(km.resolve("key"), Some("global".into()));
}

// ── Layer binding lifecycle ──────────────────────────────────────────

#[test]
fn pop_layer_deactivates_without_losing_bindings() {
    let mut km = make_km();
    km.set_layer("insert", "esc", "exit-insert-mode");
    km.push_layer("insert");
    assert_eq!(km.resolve("esc"), Some("exit-insert-mode".into()));

    km.pop_layer("insert");
    assert_eq!(km.resolve("esc"), None);

    km.push_layer("insert");
    assert_eq!(km.resolve("esc"), Some("exit-insert-mode".into()));
}

#[test]
fn set_layer_before_push_is_visible_after_push() {
    let mut km = make_km();
    km.set_layer("insert", "esc", "exit-insert-mode");
    km.set_layer("insert", "ctrl-c", "exit-insert-mode");

    assert!(!km.is_layer_active("insert"));
    assert_eq!(km.resolve("esc"), None);

    km.push_layer("insert");
    assert!(km.is_layer_active("insert"));
    assert_eq!(km.resolve("esc"),    Some("exit-insert-mode".into()));
    assert_eq!(km.resolve("ctrl-c"), Some("exit-insert-mode".into()));
}

#[test]
fn set_layer_with_global_is_same_as_set() {
    let mut km = make_km();
    km.set_layer("global", "ctrl-s", "save-buffer");
    assert_eq!(km.resolve("ctrl-s"), Some("save-buffer".into()));
    // Also check that it's in the global layer, not in layers registry
    assert_eq!(km.list_layer("global").len(), 1);
    // list_all includes global and no named layers
    let all = km.list_all();
    assert_eq!(all.len(), 1);
}

#[test]
fn unset_layer_with_global_removes_global() {
    let mut km = make_km();
    km.set("ctrl-q", "quit");
    km.unset_layer("global", "ctrl-q");
    assert_eq!(km.resolve("ctrl-q"), None);
}

#[test]
fn unset_layer_on_nonexistent_layer_is_safe() {
    let mut km = make_km();
    km.unset_layer("imaginary-layer", "some-key"); // must not panic
}

#[test]
fn set_layer_overwrites_previous_at_same_layer() {
    let mut km = make_km();
    km.set_layer("vim", "h", "cursor-left");
    km.set_layer("vim", "h", "cursor-left-alt");
    km.push_layer("vim");
    assert_eq!(km.resolve("h"), Some("cursor-left-alt".into()));
}

#[test]
fn unset_layer_removes_from_named_layer() {
    let mut km = make_km();
    km.set_layer("vim", "h", "cursor-left");
    km.push_layer("vim");
    assert_eq!(km.resolve("h"), Some("cursor-left".into()));

    km.unset_layer("vim", "h");
    assert_eq!(km.resolve("h"), None);
}

// ── Buffer-local bindings ────────────────────────────────────────────

#[test]
fn buffer_local_overrides_layer() {
    let mut km = make_km();
    km.set_layer("vim", "h", "cursor-left");
    km.push_layer("vim");
    km.set_buffer_local(42, "h", "buffer-specific");

    assert_eq!(km.resolve_for_buffer("h", Some(42)), Some("buffer-specific".into()));
    assert_eq!(km.resolve_for_buffer("h", None),     Some("cursor-left".into()));
    assert_eq!(km.resolve_for_buffer("h", Some(99)), Some("cursor-left".into()));
}

#[test]
fn buffer_local_only_affects_its_buffer() {
    let mut km = make_km();
    km.set("ctrl-s", "save");
    km.set_buffer_local(1, "ctrl-s", "buffer-1-save");
    km.set_buffer_local(2, "ctrl-s", "buffer-2-save");

    assert_eq!(km.resolve_for_buffer("ctrl-s", Some(1)), Some("buffer-1-save".into()));
    assert_eq!(km.resolve_for_buffer("ctrl-s", Some(2)), Some("buffer-2-save".into()));
    assert_eq!(km.resolve_for_buffer("ctrl-s", Some(3)), Some("save".into()));
    assert_eq!(km.resolve_for_buffer("ctrl-s", None),    Some("save".into()));
}

#[test]
fn buffer_local_resolves_when_no_layer_active() {
    let mut km = make_km();
    km.set("ctrl-s", "global-save");
    km.set_buffer_local(7, "ctrl-s", "local-save");
    // No layers active, buffer local should still win
    assert_eq!(km.resolve_for_buffer("ctrl-s", Some(7)), Some("local-save".into()));
}

#[test]
fn buffer_local_without_global_fallback() {
    let mut km = make_km();
    km.set_buffer_local(7, "special", "only-buf-7");
    assert_eq!(km.resolve_for_buffer("special", Some(7)), Some("only-buf-7".into()));
    assert_eq!(km.resolve_for_buffer("special", Some(8)), None);
    assert_eq!(km.resolve_for_buffer("special", None),    None);
}

// ── Modal editing cycles ─────────────────────────────────────────────

#[test]
fn normal_mode_hjkl_resolve() {
    let mut km = make_km();
    for (k, v) in [("h","cursor-left"),("j","cursor-down"),("k","cursor-up"),("l","cursor-right")] {
        km.set_layer("vim", k, v);
    }
    km.set_layer("vim", "i", "enter-insert-mode");
    km.push_layer("vim");

    assert_eq!(km.resolve("h"), Some("cursor-left".into()));
    assert_eq!(km.resolve("i"), Some("enter-insert-mode".into()));
    assert_eq!(km.resolve("a"), None);
}

#[test]
fn insert_mode_esc_exits() {
    let mut km = make_km();
    km.set_layer("vim",    "h",   "cursor-left");
    km.set_layer("vim",    "i",   "enter-insert-mode");
    km.set_layer("insert", "esc", "exit-insert-mode");
    km.push_layer("vim");

    km.push_layer("insert");
    assert!(km.is_layer_active("insert"));
    assert_eq!(km.resolve("esc"), Some("exit-insert-mode".into()));
    assert_eq!(km.resolve("h"),   Some("cursor-left".into()));
    assert_eq!(km.resolve("i"),   Some("enter-insert-mode".into()));

    km.pop_layer("insert");
    assert!(!km.is_layer_active("insert"));
    assert_eq!(km.resolve("esc"), None);
    assert_eq!(km.resolve("h"),   Some("cursor-left".into()));
}

#[test]
fn multiple_toggle_cycles_work() {
    let mut km = make_km();
    km.set_layer("vim",    "i",   "enter-insert-mode");
    km.set_layer("insert", "esc", "exit-insert-mode");
    km.push_layer("vim");

    for _ in 0..5 {
        km.push_layer("insert");
        assert_eq!(km.resolve("esc"), Some("exit-insert-mode".into()));
        km.pop_layer("insert");
        assert_eq!(km.resolve("esc"), None);
    }
}

// ── Introspection ────────────────────────────────────────────────────

#[test]
fn list_layer_returns_bindings() {
    let mut km = make_km();
    km.set_layer("vim", "h", "cursor-left");
    km.set_layer("vim", "j", "cursor-down");
    let bindings = km.list_layer("vim");
    assert_eq!(bindings.len(), 2);
    assert!(bindings.contains(&("h".into(), "cursor-left".into())));
    assert!(bindings.contains(&("j".into(), "cursor-down".into())));
}

#[test]
fn list_layer_returns_empty_for_nonexistent() {
    let km = make_km();
    let bindings = km.list_layer("nonexistent");
    assert!(bindings.is_empty());
}

#[test]
fn list_layer_global_returns_all_global_bindings() {
    let mut km = make_km();
    km.set("ctrl-s", "save");
    km.set("ctrl-q", "quit");
    let bindings = km.list_layer("global");
    assert_eq!(bindings.len(), 2);
    assert!(bindings.contains(&("ctrl-s".into(), "save".into())));
    assert!(bindings.contains(&("ctrl-q".into(), "quit".into())));
}

#[test]
fn list_all_includes_all_layers_and_global() {
    let mut km = make_km();
    km.set("ctrl-q", "quit");
    km.set_layer("vim",    "h", "cursor-left");
    km.set_layer("insert", "esc", "exit-insert-mode");

    let all = km.list_all();
    // global is always present, plus any layers that have bindings
    assert!(all.contains_key("global"));
    assert!(all.contains_key("vim"));
    assert!(all.contains_key("insert"));

    assert_eq!(all.get("global").unwrap().len(), 1);
    assert_eq!(all.get("vim").unwrap().len(),    1);
    assert_eq!(all.get("insert").unwrap().len(), 1);

    // Layers registered but without bindings do not appear in list_all
    // (they aren't in layers until set_layer or push_layer is called)
}

#[test]
fn list_all_returns_empty_global_when_no_bindings() {
    let km = make_km();
    let all = km.list_all();
    assert!(all.contains_key("global"));
    assert!(all.get("global").unwrap().is_empty());
    // No named layers
    assert_eq!(all.len(), 1);
}

#[test]
fn active_layers_includes_global_first() {
    let mut km = make_km();
    km.push_layer("vim");
    km.push_layer("insert");
    let active = km.active_layers();
    assert_eq!(active, vec!["global", "vim", "insert"]);
}

#[test]
fn active_layers_order_matches_push_order() {
    let mut km = make_km();
    km.push_layer("a");
    km.push_layer("b");
    km.push_layer("c");
    let active = km.active_layers();
    assert_eq!(active, vec!["global", "a", "b", "c"]);
}

#[test]
fn active_layers_only_global_when_no_layers_active() {
    let km = make_km();
    assert_eq!(km.active_layers(), vec!["global"]);
}

#[test]
fn describe_is_alias_for_resolve() {
    let mut km = make_km();
    km.set("ctrl-s", "save");
    assert_eq!(km.describe("ctrl-s"), km.resolve("ctrl-s"));
    assert_eq!(km.describe("ctrl-s"), Some("save".into()));
    assert_eq!(km.describe("unknown"), None);
}

// ── Complex integration ──────────────────────────────────────────────

#[test]
fn full_resolution_chain_global_layer_buffer() {
    let mut km = make_km();
    // Global: ctrl-s → save, ctrl-q → quit
    km.set("ctrl-s", "global-save");
    km.set("ctrl-q", "global-quit");

    // Layer "vim": h → cursor-left, ctrl-s → vim-save
    km.set_layer("vim", "h", "cursor-left");
    km.set_layer("vim", "ctrl-s", "vim-save");

    // Layer "insert": esc → exit-insert-mode
    km.set_layer("insert", "esc", "exit-insert-mode");

    // Buffer 42: ctrl-s → buf-42-save
    km.set_buffer_local(42, "ctrl-s", "buf-42-save");

    // No layers active: only global resolves
    assert_eq!(km.resolve("ctrl-s"), Some("global-save".into()));
    assert_eq!(km.resolve("ctrl-q"), Some("global-quit".into()));
    assert_eq!(km.resolve_for_buffer("ctrl-s", Some(42)), Some("buf-42-save".into()));

    // Push vim: vim overrides global for h, ctrl-s; global for ctrl-q
    km.push_layer("vim");
    assert_eq!(km.resolve("h"),       Some("cursor-left".into()));
    assert_eq!(km.resolve("ctrl-s"),  Some("vim-save".into()));
    assert_eq!(km.resolve("ctrl-q"),  Some("global-quit".into()));
    // Buffer-local still wins for buffer 42
    assert_eq!(km.resolve_for_buffer("ctrl-s", Some(42)), Some("buf-42-save".into()));

    // Push insert: insert wins for esc, but not ctrl-s or h
    km.push_layer("insert");
    assert_eq!(km.resolve("esc"),     Some("exit-insert-mode".into()));
    assert_eq!(km.resolve("ctrl-s"),  Some("vim-save".into()));      // from vim (insert doesn't bind)
    assert_eq!(km.resolve("h"),       Some("cursor-left".into()));   // from vim

    // Pop insert and vim: back to global
    km.pop_layer("insert");
    km.pop_layer("vim");
    assert_eq!(km.resolve("ctrl-s"),  Some("global-save".into()));
    assert_eq!(km.resolve("h"),       None);
}

#[test]
fn many_bindings_in_one_layer() {
    let mut km = make_km();
    for i in 0..50 {
        let key = format!("key-{}", i);
        let cmd = format!("cmd-{}", i);
        km.set_layer("big", &key, &cmd);
    }
    km.push_layer("big");
    for i in 0..50 {
        let key = format!("key-{}", i);
        let cmd = format!("cmd-{}", i);
        assert_eq!(km.resolve(&key), Some(cmd));
    }
}

#[test]
fn pop_all_layers_returns_to_global_only() {
    let mut km = make_km();
    km.set("only-global", "found");
    km.set_layer("a", "only-layer-a", "found");
    km.set_layer("b", "only-layer-b", "found");
    km.push_layer("a");
    km.push_layer("b");

    assert_eq!(km.resolve("only-global"),  Some("found".into()));
    assert_eq!(km.resolve("only-layer-a"), Some("found".into()));
    assert_eq!(km.resolve("only-layer-b"), Some("found".into()));

    km.pop_layer("b");
    assert_eq!(km.resolve("only-layer-b"), None);

    km.pop_layer("a");
    assert_eq!(km.resolve("only-layer-a"), None);
    assert_eq!(km.resolve("only-global"),  Some("found".into()));
}
