use crate::janet_bridge;

/// Helper: type a string into command mode via dispatch_key
fn type_command(ed: &mut crate::state::Editor, s: &str) {
    crate::command::execute_command(ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    for ch in s.chars() {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        crate::input::dispatch_key(ed, s);
    }
    crate::input::dispatch_key(ed, "return");
}

#[test]
fn colon_q_via_janet_fiber() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    janet_bridge::init(&mut ed);

    assert!(ed.commands.exists("command-execute"),
        "command-execute must be registered after init");

    crate::command::execute_command(&mut ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    assert!(ed.editor_mode.is_named("command"), "should be in command mode");

    crate::input::dispatch_key(&mut ed, "q");
    assert!(ed.editor_mode.minibuffer.as_ref().map(|mb| mb.input.as_str()) == Some("q"),
        "command input should be 'q'");

    crate::input::dispatch_key(&mut ed, "return");

    assert!(ed.editor_mode.is_named("normal"),
        "should be normal mode after :q");
    assert!(!ed.running, "editor should not be running after :q");
}

#[test]
fn colon_w_via_janet_fiber() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    janet_bridge::init(&mut ed);

    assert!(ed.commands.exists("command-execute"),
        "command-execute must be registered after init");

    crate::command::execute_command(&mut ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    assert!(ed.editor_mode.is_named("command"), "should be in command mode");

    crate::input::dispatch_key(&mut ed, "w");
    crate::input::dispatch_key(&mut ed, "return");

    assert!(ed.editor_mode.is_named("normal"),
        "should be normal mode after :w");
}

#[test]
fn colon_execute_unknown_does_not_crash() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    janet_bridge::init(&mut ed);

    assert!(ed.commands.exists("command-execute"));

    crate::command::execute_command(&mut ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    crate::input::dispatch_key(&mut ed, "f");
    crate::input::dispatch_key(&mut ed, "o");
    crate::input::dispatch_key(&mut ed, "o");
    crate::input::dispatch_key(&mut ed, "return");

    assert!(ed.editor_mode.is_named("normal"),
        "should be normal mode after unknown :foo");
}

#[test]
fn colon_set_via_fiber_works() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    janet_bridge::init(&mut ed);

    crate::command::execute_command(&mut ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    for ch in "set tab-width=8".chars() {
        crate::input::dispatch_key(&mut ed, &ch.to_string());
    }
    crate::input::dispatch_key(&mut ed, "return");

    assert!(ed.editor_mode.is_named("normal"),
        "should be normal mode after :set");
    assert_eq!(ed.options.get("tab-width").map(|s| s.as_str()),
        Some("8"), "tab-width should be set to 8");
}

#[test]
fn colon_execute_direct() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    janet_bridge::init(&mut ed);

    let expr = r#"(colon-execute "q")"#;
    let _result = janet_bridge::eval(expr);
    assert!(!ed.running, "running should be false after :q");
}

#[test]
fn mode_detail_keyword_lookup() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world");
    janet_bridge::init(&mut ed);

    crate::command::execute_command(&mut ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    crate::input::dispatch_key(&mut ed, "q");

    let expr = r#"(= (get (editor/mode-detail) :input "") "q")"#;
    let result = janet_bridge::eval(expr);
    assert_eq!(result, "ok", "keyword lookup in mode-detail table should work");
}

#[test]
fn colon_substitute_in_buffer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world hello");
    janet_bridge::init(&mut ed);

    crate::command::execute_command(&mut ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    for ch in "s/hello/goodbye/".chars() {
        let s = ch.to_string();
        crate::input::dispatch_key(&mut ed, &s);
    }
    std::fs::write("/tmp/magma_dbg.txt", "BEFORE_RETURN\n").ok();

    // Use RAII guard to detect unwinding
    struct DbgWrite;
    impl Drop for DbgWrite {
        fn drop(&mut self) {
            std::fs::write("/tmp/magma_dbg_guard.txt", "guard_dropped\n").ok();
        }
    }
    let _guard = DbgWrite;
    std::fs::write("/tmp/magma_dbg_guard.txt", "before_dispatch\n").ok();
    crate::input::dispatch_key(&mut ed, "return");
    std::fs::write("/tmp/magma_dbg_guard.txt", "after_dispatch\n").ok();
    drop(_guard);
    std::fs::write("/tmp/magma_dbg.txt", "AFTER_RETURN\n").ok();

    assert!(ed.editor_mode.is_named("normal"),
        "should be normal mode after :s");
    let focused = crate::input::focused_buffer_id(&ed);
    let buf = ed.buffers.get(focused).unwrap();
    assert_eq!(buf.slice(0, buf.len()), "goodbye world hello",
        "substitute should replace first occurrence only");
}

#[test]
fn colon_substitute_global() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello world hello");
    janet_bridge::init(&mut ed);

    crate::command::execute_command(&mut ed, "enter-command-mode", &std::collections::HashMap::new()).unwrap();
    for ch in "s/hello/goodbye/g".chars() {
        let s = ch.to_string();
        crate::input::dispatch_key(&mut ed, &s);
    }
    crate::input::dispatch_key(&mut ed, "return");

    assert!(ed.editor_mode.is_named("normal"),
        "should be normal mode after :s with g flag");
    let focused = crate::input::focused_buffer_id(&ed);
    let buf = ed.buffers.get(focused).unwrap();
    assert_eq!(buf.slice(0, buf.len()), "goodbye world goodbye",
        "global substitute should replace all occurrences");
}

#[test]
fn colon_q_quit_qbang() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    janet_bridge::init(&mut ed);
    assert!(ed.running);

    type_command(&mut ed, "q!");
    assert!(!ed.running, "running should be false after :q!");
    ed.running = true;

    type_command(&mut ed, "quit");
    assert!(!ed.running, "running should be false after :quit");
    ed.running = true;

    type_command(&mut ed, "q");
    assert!(!ed.running, "running should be false after :q");
}

#[test]
fn colon_wq_and_x() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    janet_bridge::init(&mut ed);
    assert!(ed.running);

    let focused = crate::input::focused_buffer_id(&ed);
    ed.buffers.get_mut(focused).unwrap().path = Some("/tmp/test_wq".to_string());
    std::fs::write("/tmp/test_wq", "").expect("cannot create /tmp/test_wq for test");

    type_command(&mut ed, "wq");
    assert!(!ed.running, "running should be false after :wq");

    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    janet_bridge::init(&mut ed);
    let focused = crate::input::focused_buffer_id(&ed);
    ed.buffers.get_mut(focused).unwrap().path = Some("/tmp/test_x".to_string());
    std::fs::write("/tmp/test_x", "").expect("cannot create /tmp/test_x for test");
    type_command(&mut ed, "x");
    assert!(!ed.running, "running should be false after :x");
}

#[test]
fn colon_write_alias() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    janet_bridge::init(&mut ed);

    type_command(&mut ed, "write /tmp/test_write_alias");
    let focused = crate::input::focused_buffer_id(&ed);
    let buf = ed.buffers.get(focused).unwrap();
    assert_eq!(buf.path, Some("/tmp/test_write_alias".to_string()),
        ":write should set buffer path");
}

#[test]
fn colon_e_edit_r_read() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    janet_bridge::init(&mut ed);

    type_command(&mut ed, "e /tmp/test_edit");
    let focused = crate::input::focused_buffer_id(&ed);
    assert!(ed.buffers.get(focused).is_some(),
        "buffer should still exist after :e");
    assert!(ed.editor_mode.is_named("normal"),
        "should be normal after :e");
}

#[test]
fn colon_shell_not_broken() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("hello");
    janet_bridge::init(&mut ed);

    type_command(&mut ed, "! echo test");
    assert!(ed.editor_mode.is_named("normal"),
        "should be normal after :! command");

    type_command(&mut ed, "!echo");
    assert!(ed.editor_mode.is_named("normal"),
        "should be normal after unknown :!echo (falls through)");
}

#[test]
fn colon_all_verb_extractions_via_eval() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = crate::tests::helpers::make_editor_with_buffer("test");
    janet_bridge::init(&mut ed);

    let tests: &[&str] = &[
        "q", "q!", "quit", "wq", "x",
        "w", "w /tmp/foo", "write /tmp/foo",
        "s/old/new/", "s /old/new/",
        "e /tmp/foo", "edit /tmp/foo",
        "r /tmp/foo", "read /tmp/foo",
        "! echo ok",
        "set tab-width=8",
    ];
    for cmd in tests {
        ed.running = true;
        let expr = format!("(try (colon-execute \"{}\") ([e] nil))", cmd);
        let result = janet_bridge::eval(&expr);
        assert_eq!(result, "ok", "colon-execute \"{}\" should not crash janet", cmd);
    }
}
