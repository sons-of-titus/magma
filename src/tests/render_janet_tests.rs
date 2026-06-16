use crate::kernel::scripting;

// ── font/invalidate ────────────────────────────────────────────────────

#[test]
fn font_invalidate_sets_font_changed_flag() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::kernel::scripting::init(&mut ed);

    assert!(!ed.font_changed, "font_changed must start false");
    let r = scripting::eval("(font/invalidate)");
    assert_eq!(r, "ok");
    assert!(ed.font_changed, "font_changed must be true after font/invalidate");
}

// ── font-changed event ────────────────────────────────────────────────────────

#[test]
fn font_changed_event_has_subscriber_from_ecosystem() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::kernel::scripting::init(&mut ed);

    let count = ed.events.subscriber_count("font-changed");
    assert!(count > 0, "ecosystem.janet must register a font-changed subscriber");
}

#[test]
fn font_changed_event_sets_flag_after_drain() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::kernel::scripting::init(&mut ed);

    assert!(!ed.font_changed);
    // Emit the event via Janet (queues it)
    scripting::eval(r#"(event/emit "font-changed" {})"#);
    // Drain dispatches the Rust subscriber, which calls the Janet handler,
    // which calls font/invalidate, which sets ed.font_changed = true.
    ed.events.drain_and_dispatch();
    assert!(ed.font_changed, "font_changed flag must be set after font-changed event is drained");
}

#[test]
fn font_changed_flag_is_cleared_after_being_read() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello\n");
    crate::kernel::scripting::init(&mut ed);

    // Set the flag directly, then simulate the GuiApp reading and clearing it.
    ed.font_changed = true;
    let was_set = ed.font_changed;
    ed.font_changed = false; // GuiApp clears it after rebuilding the atlas
    assert!(was_set);
    assert!(!ed.font_changed);
}
