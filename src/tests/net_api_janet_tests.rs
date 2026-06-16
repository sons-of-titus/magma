use crate::kernel::scripting;

fn setup_background_editor() -> crate::kernel::state::Editor {
    let mut ed = crate::kernel::state::Editor::new(Box::new(crate::kernel::storage::disk::DiskFileSystem::new()));
    crate::kernel::command::builtin::register_builtin_commands(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (bg_sender, _bg_receiver) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(crate::kernel::runtime::BackgroundHandle::new(runtime, bg_sender));
    let key = ed.create_buffer("test");
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    ed
}

// ── net/http-request ─────────────────────────────────────────────────────

#[test]
fn http_request_returns_positive_integer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(net/http-request "GET" "http://example.com")"#,
    )
    .unwrap();
    let n: i64 = r.parse().expect("http-request must return a number");
    assert!(n > 0, "request-id must be positive, got {n}");
}

#[test]
fn http_request_increments_next_net_id() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let before = ed.io.next_net_id;
    scripting::eval(r#"(net/http-request "GET" "http://example.com")"#);
    assert_eq!(ed.io.next_net_id, before + 1, "next_net_id must increment after http-request");
}

// ── net/http-get ─────────────────────────────────────────────────────────

#[test]
fn http_get_returns_integer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(net/http-get "http://example.com")"#)
        .unwrap();
    let n: i64 = r.parse().expect("http-get must return a number");
    assert!(n > 0, "request-id must be positive");
}

// ── net/http-post ─────────────────────────────────────────────────────────

#[test]
fn http_post_returns_integer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(net/http-post "http://example.com" "test-body")"#,
    )
    .unwrap();
    let n: i64 = r.parse().expect("http-post must return a number");
    assert!(n > 0, "request-id must be positive");
}

// ── net/tcp-listen / net/tcp-stop ─────────────────────────────────────────

#[test]
fn tcp_listen_adds_entry_to_net_servers() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(net/tcp-listen 17891)"#).unwrap();
    let server_id: u64 = r.parse().expect("tcp-listen must return a number");
    assert!(server_id > 0, "server-id must be positive");
    assert!(
        ed.io.net_servers.contains_key(&server_id),
        "net_servers must contain the new server entry"
    );
    scripting::eval(&format!("(net/tcp-stop {server_id})"));
    assert!(
        !ed.io.net_servers.contains_key(&server_id),
        "net_servers must not contain the server after tcp-stop"
    );
}

#[test]
fn tcp_listen_returns_positive_server_id() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(net/tcp-listen 17892)"#).unwrap();
    let sid: u64 = r.parse().expect("server-id must be an integer");
    assert!(sid >= 1);
    scripting::eval(&format!("(net/tcp-stop {sid})"));
}

// ── net/tcp-broadcast on non-existent server ──────────────────────────────

#[test]
fn tcp_broadcast_non_existent_server_returns_nil_without_crash() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(net/tcp-broadcast 9999999 "hello")"#,
    )
    .unwrap();
    assert_eq!(r, "nil", "broadcast to non-existent server must return nil");
}

// ── net/tcp-send-to on non-existent server ────────────────────────────────

#[test]
fn tcp_send_to_non_existent_server_returns_nil_without_crash() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(net/tcp-send-to 9999999 1 "hello")"#,
    )
    .unwrap();
    assert_eq!(r, "nil");
}

// ── Collab colon verbs ────────────────────────────────────────────────────

#[test]
fn collab_start_colon_verb_is_registered() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(truthy? (get *colon-plugins* "collab-start"))"#,
    )
    .unwrap();
    assert_eq!(r, "true", "collab-start colon verb must be registered");
}

#[test]
fn collab_connect_colon_verb_is_registered() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(truthy? (get *colon-plugins* "collab-connect"))"#,
    )
    .unwrap();
    assert_eq!(r, "true", "collab-connect colon verb must be registered");
}

#[test]
fn collab_stop_colon_verb_is_registered() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(truthy? (get *colon-plugins* "collab-stop"))"#,
    )
    .unwrap();
    assert_eq!(r, "true", "collab-stop colon verb must be registered");
}

// ── SSH FS colon verbs ────────────────────────────────────────────────────

#[test]
fn ssh_colon_verb_is_registered() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(truthy? (get *colon-plugins* "ssh"))"#,
    )
    .unwrap();
    assert_eq!(r, "true", "ssh colon verb must be registered");
}

#[test]
fn vfs_open_colon_verb_is_registered() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(truthy? (get *colon-plugins* "vfs-open"))"#,
    )
    .unwrap();
    assert_eq!(r, "true", "vfs-open colon verb must be registered");
}

// ── vfs/open SSH URI parsing ──────────────────────────────────────────────

#[test]
fn vfs_open_ssh_uri_does_not_crash() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval(r#"(vfs/open "ssh://user@host/tmp/file.txt")"#);
    assert_eq!(r, "ok", "vfs/open with ssh URI must not crash the Janet VM");
}

// ── ssh-fs/open function exists ───────────────────────────────────────────

#[test]
fn ssh_fs_open_function_exists() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(truthy? ssh-fs/open)"#).unwrap();
    assert_eq!(r, "true", "ssh-fs/open must be defined in the Janet environment");
}

// ── ssh-fs/read function exists ───────────────────────────────────────────

#[test]
fn ssh_fs_read_function_exists() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(truthy? ssh-fs/read)"#).unwrap();
    assert_eq!(r, "true", "ssh-fs/read must be defined in the Janet environment");
}

// ── net/on-response function exists ──────────────────────────────────────

#[test]
fn net_on_response_function_exists() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(truthy? net/on-response)"#).unwrap();
    assert_eq!(r, "true", "net/on-response must be defined");
}

// ── *net-pending* table exists ────────────────────────────────────────────

#[test]
fn net_pending_table_exists() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(table? *net-pending*)"#).unwrap();
    assert_eq!(r, "true", "*net-pending* must be a table");
}

// ── collab/encode-decode round-trip (pure Janet) ──────────────────────────

#[test]
fn collab_peer_cursors_table_exists() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(table? *collab-peer-cursors*)"#).unwrap();
    assert_eq!(r, "true", "*collab-peer-cursors* must be a table");
}

// ── collab server id starts nil ───────────────────────────────────────────

#[test]
fn collab_server_id_starts_nil() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(r#"(nil? *collab-server-id*)"#).unwrap();
    assert_eq!(r, "true", "*collab-server-id* must start as nil");
}

// ── tcp-connect increments next_net_id ───────────────────────────────────

#[test]
fn tcp_connect_increments_next_net_id() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let before = ed.io.next_net_id;
    scripting::eval(r#"(net/tcp-connect "127.0.0.1" 19999)"#);
    assert_eq!(
        ed.io.next_net_id,
        before + 1,
        "next_net_id must increment after tcp-connect"
    );
}

// ── tcp-connect returns a positive integer ────────────────────────────────

#[test]
fn tcp_connect_returns_positive_integer() {
    let _lock = crate::tests::helpers::acquire_janet_lock();
    let mut ed = setup_background_editor();
    crate::kernel::scripting::init(&mut ed);
    let r = scripting::eval_result(
        r#"(net/tcp-connect "127.0.0.1" 19998)"#,
    )
    .unwrap();
    let n: i64 = r.parse().expect("tcp-connect must return a number");
    assert!(n > 0, "conn-id must be positive");
    scripting::eval(&format!("(net/tcp-close {n})"));
}
