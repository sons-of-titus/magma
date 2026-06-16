/// LSP integration — async language server protocol using tokio.
///
/// Each language server runs as a child process. Communication is via JSON-RPC
/// over stdin/stdout. Diagnostics and completions are sent back to the main
/// thread through the background event channel.
pub mod client;
pub mod parse;

pub use client::LspManager;
pub use client::pending_map;
