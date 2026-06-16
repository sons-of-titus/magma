//! Pure-Rust tests for the Sprint 13 network API state (no Janet VM needed).

use crate::state::Editor;
use crate::tests::helpers;

// ── net_connections ───────────────────────────────────────────────────────────

#[test]
fn net_connections_starts_empty() {
    let ed = helpers::make_editor();
    assert!(ed.io.net_connections.is_empty(), "net_connections must start empty");
}

// ── net_servers ───────────────────────────────────────────────────────────────

#[test]
fn net_servers_starts_empty() {
    let ed = helpers::make_editor();
    assert!(ed.io.net_servers.is_empty(), "net_servers must start empty");
}

// ── next_net_id ───────────────────────────────────────────────────────────────

#[test]
fn next_net_id_starts_at_one() {
    let ed = helpers::make_editor();
    assert_eq!(ed.io.next_net_id, 1, "next_net_id must start at 1");
}

// ── Sprint 12 primitives still registered ─────────────────────────────────────

#[test]
fn eval_region_is_still_registered() {
    let ed = helpers::make_editor();
    assert!(
        ed.commands.get_entry("eval-region").is_some(),
        "eval-region must remain registered after Sprint 13"
    );
}

#[test]
fn eval_buffer_is_still_registered() {
    let ed = helpers::make_editor();
    assert!(
        ed.commands.get_entry("eval-buffer").is_some(),
        "eval-buffer must remain registered after Sprint 13"
    );
}

// ── NetServer struct fields ────────────────────────────────────────────────────

#[test]
fn net_server_clients_starts_empty_on_creation() {
    use std::sync::{Arc, atomic::AtomicBool};

    let srv = crate::net::NetServer {
        port: 9999,
        running: Arc::new(AtomicBool::new(true)),
        clients: std::collections::HashMap::new(),
        next_client_id: 1,
    };
    assert!(srv.clients.is_empty());
    assert_eq!(srv.next_client_id, 1);
    assert_eq!(srv.port, 9999);
}

// ── NetConnection struct fields ───────────────────────────────────────────────

#[test]
fn net_connection_not_connected_on_creation() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let conn = crate::net::NetConnection {
        host: "localhost".to_string(),
        port: 8080,
        connected: false,
        write_tx: tx,
    };
    assert!(!conn.connected);
    assert_eq!(conn.host, "localhost");
    assert_eq!(conn.port, 8080);
}
