//! The managed-download surface: manifest gating, bundled `:command`
//! resolution, the pinned-download trigger, and `install/find`.

#[macro_use]
mod helpers;
use scripting;

fn root_str(path: &std::path::Path) -> String {
    path.to_str().unwrap().replace('\\', "/")
}

const FAKE_SERVER: &str = r#"#!/usr/bin/env python3
import sys, json

def read_msg():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            raise EOFError("stdin closed")
        if line in (b"\r\n", b"\n"):
            break
        text = line.decode("utf-8", "replace")
        key, _, value = text.partition(":")
        headers[key.strip().lower()] = value.strip()
    n = int(headers["content-length"])
    return json.loads(sys.stdin.buffer.read(n))

def write_msg(obj):
    body = json.dumps(obj).encode()
    sys.stdout.buffer.write(b"Content-Length: %d\r\n\r\n" % len(body))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()

while True:
    try:
        msg = read_msg()
    except Exception:
        break
    method = msg.get("method")
    if method == "initialize":
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": {"capabilities": {}}})
    elif "id" in msg:
        write_msg({"jsonrpc": "2.0", "id": msg["id"], "result": None})
"#;

fn fixture_plugin(tag_for_name: &str) -> String {
    // A plugin root with the fake server under servers/, platform-suffixed.
    let dir =
        std::env::temp_dir().join(format!("magma_bundled_{}_{}", tag_for_name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let servers = dir.join("servers");
    std::fs::create_dir_all(&servers).unwrap();
    let tag = if cfg!(target_os = "macos") { "macos" } else { "linux" };
    let script = servers.join(format!("fake-server.{tag}"));
    std::fs::write(&script, FAKE_SERVER).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir.to_str().unwrap().replace('\\', "/")
}

/// Register the zzbin language with a full `:lsp` spec literal.
fn register_zzbin(spec_literal: &str) {
    assert_eq!(
        scripting::eval(r#"(major-mode/define "zzbin-mode" {:parent "prog"})"#),
        "ok"
    );
    assert_eq!(
        scripting::eval(r#"(language/define {:id "zzbin" :extensions ["zbd"]})"#),
        "ok"
    );
    assert_eq!(
        scripting::eval(&format!(
            r#"(lsp/register-language-spec "zzbin" {spec_literal})"#
        )),
        "ok"
    );
}

fn attach_buffer_of_language(ed: &mut kernel::state::Editor, name: &str) {
    let buf = ed.create_buffer_from_str(name, "let x = 1;\n");
    let path = std::env::temp_dir().join(name);
    ed.buffers
        .get(buf)
        .unwrap()
        .lock()
        .unwrap()
        .path = Some(path.to_string_lossy().into_owned());
    let ok = scripting::eval_result(&format!(
        r#"(do (event/emit "buffer-created" {{"buffer-id" "{buf}" "path" "{}"}}) true)"#,
        path.to_str().unwrap().replace('\\', "/")
    ))
    .unwrap();
    assert_eq!(ok, "true");
    ed.dispatch_events_scoped();
}

// ── manifest gating ──────────────────────────────────────────────────────────

#[test]
fn manifest_install_requires_the_network_capability() {
    janet_test!(_ed, {
        let result = scripting::eval_result(
            r#"(try (plugin/define {:name "zz-inst-bad" :version "0.1.0" :languages ["zzinst"]
                     :lsp {"zzinst" {:single-file true
                                     :install {:url "https://x/y.gz" :sha256 "00" :format "gz" :bin "y"}}}
                     :capabilities [:process-execution]})
               ([e] e))"#,
        )
        .unwrap_or_default();
        assert!(
            result.contains("network"),
            "must name the missing capability: {result}"
        );
    });
}

// ── bundled :command ─────────────────────────────────────────────────────────

#[test]
fn bundled_command_starts_a_server_without_the_option() {
    janet_test!(ed, {
        let root = fixture_plugin("resolve");
        register_zzbin(&format!(
            r#"{{:single-file true :command "servers/fake-server" :plugin-root "{root}"}}"#
        ));
        attach_buffer_of_language(&mut ed, "x.zbd");
        // :starting means resolve-command returned the bundled path —
        // no option was consulted, nothing was downloaded.  (string :kw)
        // drops the colon.
        let state = scripting::eval_result(r#"(string (get *lsp-servers* "zzbin"))"#)
            .unwrap_or_default();
        assert_eq!(state, r#""starting""#, "bundled server must be starting");
    });
}

#[test]
fn bundled_server_reaches_ready_through_the_full_attach_path() {
    let root = fixture_plugin("flow");
    let file = std::env::temp_dir().join(format!("magma_zzbin_{}.zbd", std::process::id()));
    std::fs::write(&file, "let x = 1;\n").unwrap();
    let _lock = helpers::acquire_janet_lock();
    let mut ed = helpers::make_editor();
    scripting::init(&mut ed);
    let _editor_scope = scripting::EditorScope::new(&mut ed);
    let runtime = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    ed.background = Some(kernel::runtime::BackgroundHandle::new(runtime, tx));

    register_zzbin(&format!(
        r#"{{:single-file true :command "servers/fake-server" :plugin-root "{root}"}}"#
    ));
    // The bundle wins WITHOUT touching $PATH — resolution is plugin-relative.
    let path = file.to_str().unwrap().replace('\\', "/");
    let mut args = std::collections::HashMap::new();
    args.insert(
        "path".to_string(),
        kernel::command::args::ArgValue::Path(path.clone()),
    );
    kernel::command::execute_command(&mut ed, "open-file", &args).unwrap();
    ed.events.drain_and_dispatch();

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut ready = false;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = rx.try_recv() {
            let is_started = matches!(
                &ev,
                kernel::runtime::BackgroundEvent::Lsp(kernel::semantic::Event::Started { .. })
            );
            kernel::runtime::process_background_event(&mut ed, ev);
            if is_started {
                ed.events.drain_and_dispatch();
            }
        }
        ready = scripting::eval_result(r#"(string (= :ready (get *lsp-servers* "zzbin")))"#)
            .unwrap_or_default()
            == r#""true""#;
        if ready {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        ready,
        "the bundled server must reach :ready with no PATH or option help"
    );
    std::fs::remove_file(&file).ok();
}

// ── the pinned download ──────────────────────────────────────────────────────

const ZZ_INSTALL_SPEC: &str = r#":install {:url "https://invalid.invalid/zz-server.gz" :sha256 "abcdef00" :format "gz" :bin "zz-server"}"#;

#[test]
fn auto_install_on_starts_a_fetch_for_a_missing_server() {
    janet_test!(ed, {
        // No :command and no shipped server — only the install pin.
        register_zzbin(&format!(r#"{{:single-file true {ZZ_INSTALL_SPEC}}}"#));
        scripting::eval(r#"(option/set "servers-auto-install" "true")"#);
        attach_buffer_of_language(&mut ed, "y.zbd");
        let in_flight = scripting::eval_result(r#"(string (install/in-flight? "servers/zzbin"))"#)
            .unwrap_or_default();
        assert_eq!(in_flight, r#""true""#, "the fetch must be in flight");
        // No command resolved, so nothing tried to spawn.
        let state = scripting::eval_result(r#"(describe (get *lsp-servers* "zzbin"))"#)
            .unwrap_or_default();
        assert_eq!(state, r#""nil""#);
        let _ = ed;
    });
}

#[test]
fn auto_install_off_reports_missing_and_fetches_nothing() {
    janet_test!(ed, {
        register_zzbin(&format!(r#"{{:single-file true {ZZ_INSTALL_SPEC}}}"#));
        scripting::eval(r#"(option/set "servers-auto-install" "false")"#);
        attach_buffer_of_language(&mut ed, "z.zbd");
        let in_flight = scripting::eval_result(r#"(string (install/in-flight? "servers/zzbin"))"#)
            .unwrap_or_default();
        assert_eq!(in_flight, r#""false""#, "nothing may fetch when opted out");
        let cards = scripting::eval_result(
            r#"(string/join (map |(get $ :title) (notifications)) "|")"#,
        )
        .unwrap_or_default();
        assert!(
            cards.contains("Language server not found"),
            "the opt-out must not go silent: {cards}"
        );
        let _ = ed;
    });
}

// ── install/find ─────────────────────────────────────────────────────────────

#[test]
fn install_find_is_nil_for_an_empty_slot() {
    janet_test!(_ed, {
        let got = scripting::eval_result(
            r#"(describe (install/find "servers" "zz-empty-slot" "zz-server"))"#,
        )
        .unwrap_or_default();
        assert_eq!(got, r#""nil""#);
    });
}

// ── toolchain install pins ───────────────────────────────────────────────────

#[test]
fn toolchain_install_pin_round_trips_through_registered() {
    janet_test!(_ed, {
        assert_eq!(
            scripting::eval(
                r#"(toolchain/register {:id "zz.tc" :language "zzlang" :detect ["zz.marker"]
                     :actions {"build" {:program "zz-tool" :args ["build"]}}
                     :install {:url "https://x/zz.tar.gz" :sha256 "abcd" :format "tar.gz" :bin "zz-tool"}})"#
            ),
            "ok"
        );
        let got = scripting::eval_result(
            r#"(let [tc (find |(= (get $ :id) "zz.tc") (toolchain/registered))
                   i (get tc :install)]
               (string (get i :format) "|" (get i :bin)))"#,
        )
        .unwrap_or_default();
        assert_eq!(got, r#""tar.gz|zz-tool""#);
    });
}
