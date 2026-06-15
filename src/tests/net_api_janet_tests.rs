//! Janet API tests for Sprint 13: net/http-*, net/tcp-*, collab, ssh_fs.

#[cfg(feature = "janet")]
mod tests {
    use std::sync::Arc;

    use crate::buffer::Buffer;
    use crate::command::builtin;
    use crate::fs::disk::DiskFileSystem;
    use crate::janet_bridge;
    use crate::runtime::BackgroundHandle;
    use crate::state::id::BufferId;
    use crate::state::Editor;

    fn make_editor() -> Editor {
        let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
        builtin::register_builtin_commands(&mut ed);
        // Need a background runtime for net/http-* and net/tcp-*
        let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
        let (bg_sender, _bg_receiver) = tokio::sync::mpsc::unbounded_channel();
        let bg_handle = BackgroundHandle::new(runtime, bg_sender);
        ed.background = Some(bg_handle);
        let id = ed.allocate_buffer_id();
        let buf = Buffer::new(BufferId(id), "test");
        let entry = ed.buffers.vacant_entry();
        let key = entry.key();
        entry.insert(buf);
        if let Some(win) = ed.windows.focused_window_mut() {
            win.buffer_id = Some(key);
        }
        ed
    }

    // ── net/http-request ─────────────────────────────────────────────────────

    #[test]
    fn http_request_returns_positive_integer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // We only check that the function returns a positive integer; we do not
        // wait for the response since the test environment has no network.
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(net/http-request "GET" "http://example.com")"#,
        )
        .unwrap();
        let n: i64 = r.parse().expect("http-request must return a number");
        assert!(n > 0, "request-id must be positive, got {n}");
    }

    #[test]
    fn http_request_increments_next_net_id() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let before = ed.io.next_net_id;
        janet_bridge::eval(&mut ed, r#"(net/http-request "GET" "http://example.com")"#);
        assert_eq!(ed.io.next_net_id, before + 1, "next_net_id must increment after http-request");
    }

    // ── net/http-get ─────────────────────────────────────────────────────────

    #[test]
    fn http_get_returns_integer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(net/http-get "http://example.com")"#)
            .unwrap();
        let n: i64 = r.parse().expect("http-get must return a number");
        assert!(n > 0, "request-id must be positive");
    }

    // ── net/http-post ─────────────────────────────────────────────────────────

    #[test]
    fn http_post_returns_integer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(net/http-post "http://example.com" "test-body")"#,
        )
        .unwrap();
        let n: i64 = r.parse().expect("http-post must return a number");
        assert!(n > 0, "request-id must be positive");
    }

    // ── net/tcp-listen / net/tcp-stop ─────────────────────────────────────────

    #[test]
    fn tcp_listen_adds_entry_to_net_servers() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // Use a high, unusual port to avoid conflicts during testing
        let r = janet_bridge::eval_result(&mut ed, r#"(net/tcp-listen 17891)"#).unwrap();
        let server_id: u64 = r.parse().expect("tcp-listen must return a number");
        assert!(server_id > 0, "server-id must be positive");
        assert!(
            ed.io.net_servers.contains_key(&server_id),
            "net_servers must contain the new server entry"
        );
        // Clean up: stop the server
        janet_bridge::eval(&mut ed, &format!("(net/tcp-stop {server_id})"));
        assert!(
            !ed.io.net_servers.contains_key(&server_id),
            "net_servers must not contain the server after tcp-stop"
        );
    }

    #[test]
    fn tcp_listen_returns_positive_server_id() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(net/tcp-listen 17892)"#).unwrap();
        let sid: u64 = r.parse().expect("server-id must be an integer");
        assert!(sid >= 1);
        janet_bridge::eval(&mut ed, &format!("(net/tcp-stop {sid})"));
    }

    // ── net/tcp-broadcast on non-existent server ──────────────────────────────

    #[test]
    fn tcp_broadcast_non_existent_server_returns_nil_without_crash() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // 9999999 is very unlikely to be a valid server id
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(net/tcp-broadcast 9999999 "hello")"#,
        )
        .unwrap();
        // Should return nil and not crash
        assert_eq!(r, "nil", "broadcast to non-existent server must return nil");
    }

    // ── net/tcp-send-to on non-existent server ────────────────────────────────

    #[test]
    fn tcp_send_to_non_existent_server_returns_nil_without_crash() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(net/tcp-send-to 9999999 1 "hello")"#,
        )
        .unwrap();
        assert_eq!(r, "nil");
    }

    // ── Collab colon verbs ────────────────────────────────────────────────────

    #[test]
    fn collab_start_colon_verb_is_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(truthy? (get *colon-plugins* "collab-start"))"#,
        )
        .unwrap();
        assert_eq!(r, "true", "collab-start colon verb must be registered");
    }

    #[test]
    fn collab_connect_colon_verb_is_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(truthy? (get *colon-plugins* "collab-connect"))"#,
        )
        .unwrap();
        assert_eq!(r, "true", "collab-connect colon verb must be registered");
    }

    #[test]
    fn collab_stop_colon_verb_is_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(truthy? (get *colon-plugins* "collab-stop"))"#,
        )
        .unwrap();
        assert_eq!(r, "true", "collab-stop colon verb must be registered");
    }

    // ── SSH FS colon verbs ────────────────────────────────────────────────────

    #[test]
    fn ssh_colon_verb_is_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(truthy? (get *colon-plugins* "ssh"))"#,
        )
        .unwrap();
        assert_eq!(r, "true", "ssh colon verb must be registered");
    }

    #[test]
    fn vfs_open_colon_verb_is_registered() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(truthy? (get *colon-plugins* "vfs-open"))"#,
        )
        .unwrap();
        assert_eq!(r, "true", "vfs-open colon verb must be registered");
    }

    // ── vfs/open SSH URI parsing ──────────────────────────────────────────────

    #[test]
    fn vfs_open_ssh_uri_does_not_crash() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        // This will attempt to spawn an SSH process which will fail silently —
        // we just verify the Janet function does not panic/crash the VM.
        let r = janet_bridge::eval(&mut ed, r#"(vfs/open "ssh://user@host/tmp/file.txt")"#);
        // Janet eval must complete (return "ok" — even if the ssh process fails)
        assert_eq!(r, "ok", "vfs/open with ssh URI must not crash the Janet VM");
    }

    // ── ssh-fs/open function exists ───────────────────────────────────────────

    #[test]
    fn ssh_fs_open_function_exists() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(truthy? ssh-fs/open)"#).unwrap();
        assert_eq!(r, "true", "ssh-fs/open must be defined in the Janet environment");
    }

    // ── ssh-fs/read function exists ───────────────────────────────────────────

    #[test]
    fn ssh_fs_read_function_exists() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(truthy? ssh-fs/read)"#).unwrap();
        assert_eq!(r, "true", "ssh-fs/read must be defined in the Janet environment");
    }

    // ── net/on-response function exists ──────────────────────────────────────

    #[test]
    fn net_on_response_function_exists() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(truthy? net/on-response)"#).unwrap();
        assert_eq!(r, "true", "net/on-response must be defined");
    }

    // ── *net-pending* table exists ────────────────────────────────────────────

    #[test]
    fn net_pending_table_exists() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(table? *net-pending*)"#).unwrap();
        assert_eq!(r, "true", "*net-pending* must be a table");
    }

    // ── collab/encode-decode round-trip (pure Janet) ──────────────────────────

    #[test]
    fn collab_peer_cursors_table_exists() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(table? *collab-peer-cursors*)"#).unwrap();
        assert_eq!(r, "true", "*collab-peer-cursors* must be a table");
    }

    // ── collab server id starts nil ───────────────────────────────────────────

    #[test]
    fn collab_server_id_starts_nil() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(&mut ed, r#"(nil? *collab-server-id*)"#).unwrap();
        assert_eq!(r, "true", "*collab-server-id* must start as nil");
    }

    // ── tcp-connect increments next_net_id ───────────────────────────────────

    #[test]
    fn tcp_connect_increments_next_net_id() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let before = ed.io.next_net_id;
        // tcp-connect to a non-existent host (will fail async, but the ID is allocated sync)
        janet_bridge::eval(&mut ed, r#"(net/tcp-connect "127.0.0.1" 19999)"#);
        assert_eq!(
            ed.io.next_net_id,
            before + 1,
            "next_net_id must increment after tcp-connect"
        );
    }

    // ── tcp-connect returns a positive integer ────────────────────────────────

    #[test]
    fn tcp_connect_returns_positive_integer() {
        let _lock = janet_bridge::JANET_VM_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut ed = make_editor();
        janet_bridge::init(&mut ed);
        let r = janet_bridge::eval_result(
            &mut ed,
            r#"(net/tcp-connect "127.0.0.1" 19998)"#,
        )
        .unwrap();
        let n: i64 = r.parse().expect("tcp-connect must return a number");
        assert!(n > 0, "conn-id must be positive");
        // Clean up
        janet_bridge::eval(&mut ed, &format!("(net/tcp-close {n})"));
    }
}
