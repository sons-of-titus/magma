//! `BackgroundHandle` — the kernel's specialization of `bgpool::Pool` over
//! `BackgroundEvent`, plus the typed file read/write helpers.
//!
//! Lane scheduling and coalescing live in the `bgpool` crate; this file only
//! knows which event vocabulary the editor uses.

use std::sync::Arc;

use tokio::runtime::Runtime;
use tokio::sync::mpsc;

use super::BackgroundEvent;

pub use bgpool::Lane;

/// Handle for spawning background tasks and sending events to the main thread.
/// Cloneable — all clones share the same runtime and event channel.
#[derive(Clone)]
pub struct BackgroundHandle {
    pool: bgpool::Pool<BackgroundEvent>,
}

impl std::ops::Deref for BackgroundHandle {
    type Target = bgpool::Pool<BackgroundEvent>;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

impl semantic::lsp::EventSink for BackgroundHandle {
    fn send(&self, event: semantic::lsp::Event) {
        self.send_event(BackgroundEvent::Lsp(event));
    }
}

impl crate::debug::EventSink for BackgroundHandle {
    fn send(&self, event: crate::debug::Event) {
        self.send_event(BackgroundEvent::Dap(event));
    }
}

impl BackgroundHandle {
    pub fn new(runtime: Arc<Runtime>, sender: mpsc::UnboundedSender<BackgroundEvent>) -> Self {
        Self {
            pool: bgpool::Pool::new(runtime, sender),
        }
    }

    pub fn read_file(&self, path: String, request_id: u64) {
        let sender = self.sender.clone();
        self.spawn_blocking(move || {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string());
            let _ = sender.send(BackgroundEvent::FileReadResult {
                path,
                content,
                request_id,
            });
        });
    }

    pub fn write_file(&self, path: String, content: String, request_id: u64) {
        let sender = self.sender.clone();
        self.spawn_blocking(move || {
            let result = (|| -> Result<(), String> {
                if let Some(parent) = std::path::Path::new(&path).parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::write(&path, &content).map_err(|e| e.to_string())?;
                Ok(())
            })();
            let _ = sender.send(BackgroundEvent::FileWriteResult {
                path,
                result,
                request_id,
            });
        });
    }
}
