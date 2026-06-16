//! Network connection and server state types.
//!
//! Rust provides the primitive types; net_api.rs registers the Janet C functions.
//! Janet owns all behavior — deciding when to open connections, how to handle
//! received data, and what to do with server clients.

use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::mpsc::UnboundedSender;

/// State for an outbound TCP connection managed by `net/tcp-connect`.
pub struct NetConnection {
    /// Remote hostname or IP.
    pub host: String,
    /// Remote port.
    pub port: u16,
    /// True after `TcpConnected` has been emitted.
    pub connected: bool,
    /// Channel sender used by `net/tcp-send` to push lines to the write task.
    /// Dropping this channel signals the write task to shut down.
    pub write_tx: UnboundedSender<String>,
}

/// State for a listening TCP server managed by `net/tcp-listen`.
pub struct NetServer {
    /// Port the server is bound to.
    pub port: u16,
    /// Set to `false` by `net/tcp-stop` to shut down the accept loop.
    pub running: Arc<AtomicBool>,
    /// Per-client write channels, keyed by client ID.
    /// Dropping a channel causes the write task for that client to exit.
    pub clients: HashMap<u64, UnboundedSender<String>>,
    /// Counter for the next client ID (managed from the async task, mirrored here
    /// for consistent key assignment in `TcpClientConnected` events).
    pub next_client_id: u64,
}
