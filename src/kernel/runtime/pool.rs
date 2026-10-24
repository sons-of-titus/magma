//! The background pool — tokio runtime handle, priority lanes, and the
//! blocking/async submission surface.  Split from the event enum (R-22):
//! the pool is its own abstraction and this file is the only place that
//! knows how work is scheduled.

use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};

use super::BackgroundEvent;

/// Handle for spawning background tasks and sending events to the main thread.
/// Cloneable — all clones share the same runtime and event channel.
#[derive(Clone)]
pub struct BackgroundHandle {
    runtime: Arc<Runtime>,
    /// Channel to send events to the main thread event loop.
    pub sender: mpsc::UnboundedSender<BackgroundEvent>,
    /// Priority lanes for background work (rework.md §3).  P0 interactive
    /// work never enters this struct — it runs inline on the foreground.
    lanes: Arc<Lanes>,
}

/// Bounded concurrency per priority class, plus coalescing: a queued job
/// whose coalesce key was superseded skips itself instead of doing work
/// whose result is already stale.
struct Lanes {
    foreground: Arc<tokio::sync::Semaphore>,
    indexing: Arc<tokio::sync::Semaphore>,
    maintenance: Arc<tokio::sync::Semaphore>,
    superseded: Mutex<std::collections::HashMap<(Lane, String), u64>>,
    tickets: std::sync::atomic::AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lane {
    /// P1: visible-buffer work (parses, completion scans). 2 slots.
    Foreground,
    /// P2: project indexing, symbol extraction. cpu-1 slots.
    Indexing,
    /// P3: persistence flushes, refreshes. 1 slot.
    Maintenance,
}

impl Lane {
    fn semaphore(self, lanes: &Lanes) -> Arc<tokio::sync::Semaphore> {
        match self {
            Lane::Foreground => lanes.foreground.clone(),
            Lane::Indexing => lanes.indexing.clone(),
            Lane::Maintenance => lanes.maintenance.clone(),
        }
    }

    fn slots(&self) -> usize {
        match self {
            Lane::Foreground => 2,
            Lane::Indexing => (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) - 1).max(1),
            Lane::Maintenance => 1,
        }
    }
}

impl BackgroundHandle {
    pub fn new(runtime: Arc<Runtime>, sender: mpsc::UnboundedSender<BackgroundEvent>) -> Self {
        let lanes = Lanes {
            foreground: Arc::new(tokio::sync::Semaphore::new(Lane::Foreground.slots())),
            indexing: Arc::new(tokio::sync::Semaphore::new(Lane::Indexing.slots())),
            maintenance: Arc::new(tokio::sync::Semaphore::new(Lane::Maintenance.slots())),
            superseded: Mutex::new(std::collections::HashMap::new()),
            tickets: std::sync::atomic::AtomicU64::new(0),
        };
        Self { runtime, sender, lanes: Arc::new(lanes) }
    }

    /// Submit blocking work on a priority lane with an optional coalesce
    /// key.  A queued job whose key was superseded by a later submission
    /// exits when it finally gets a slot — newer work wins over stale.
    pub fn spawn_on<F>(&self, lane: Lane, coalesce: Option<&str>, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let ticket = self.lanes.tickets.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let coalesce_key = coalesce.map(|k| (lane, k.to_string()));
        if let Some(key) = &coalesce_key {
            let mut guard = self.lanes.superseded.lock().unwrap_or_else(|e| e.into_inner());
            guard.insert(key.clone(), ticket);
        }
        let semaphore = lane.semaphore(&self.lanes);
        let lanes = self.lanes.clone();
        self.spawn_blocking(move || {
            // Coalescing: a newer ticket for this key means the input is
            // already outdated — skip without occupying a lane slot.
            if let Some(key) = &coalesce_key {
                let guard = lanes.superseded.lock().unwrap_or_else(|e| e.into_inner());
                if guard.get(key).is_some_and(|&current| current != ticket) {
                    return;
                }
            }
            // spawn_blocking threads carry a runtime Handle; block on the
            // lane's semaphore there.
            let Ok(permit) = tokio::runtime::Handle::current().block_on(semaphore.acquire()) else {
                return;
            };
            f();
            drop(permit);
        });
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(future);
    }

    pub fn spawn_blocking<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.runtime.spawn_blocking(f);
    }

    pub fn send_event(&self, event: BackgroundEvent) {
        let _ = self.sender.send(event);
    }

    /// Synchronously execute a blocking closure on the runtime's blocking thread
    /// pool, blocking the calling thread until complete.
    /// Used by the Janet bridge to offload potentially-blocking I/O.
    pub fn block_on<F, T>(&self, f: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.runtime.spawn_blocking(move || {
            let _ = tx.send(f());
        });
        rx.blocking_recv().expect("BackgroundHandle::block_on: task panicked")
    }

    pub fn read_file(&self, path: String, request_id: u64) {
        let sender = self.sender.clone();
        self.spawn_blocking(move || {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string());
            let _ = sender.send(BackgroundEvent::FileReadResult { path, content, request_id });
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
            let _ = sender.send(BackgroundEvent::FileWriteResult { path, result, request_id });
        });
    }
}
