//! Background work pool: a tokio runtime handle with priority lanes,
//! coalescing, and an event channel to the foreground thread.
//!
//! The pool is generic over the event type, so it knows nothing about the
//! editor's event vocabulary. Scheduling policy lives here; the meaning of
//! the events lives with whoever instantiates the pool.

use std::sync::{Arc, Mutex};

use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// Handle for spawning background work and sending events to the foreground.
/// Cloneable — all clones share the same runtime, lanes, and event channel.
pub struct Pool<E> {
    runtime: Arc<Runtime>,
    /// Channel to the foreground thread's event loop.
    pub sender: mpsc::UnboundedSender<E>,
    lanes: Arc<Lanes>,
}

impl<E> Clone for Pool<E> {
    fn clone(&self) -> Self {
        Self {
            runtime: self.runtime.clone(),
            sender: self.sender.clone(),
            lanes: self.lanes.clone(),
        }
    }
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
            Lane::Indexing => {
                (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) - 1).max(1)
            }
            Lane::Maintenance => 1,
        }
    }
}

impl<E> Pool<E> {
    pub fn new(runtime: Arc<Runtime>, sender: mpsc::UnboundedSender<E>) -> Self {
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
    /// key. A queued job whose key was superseded by a later submission
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

    pub fn send_event(&self, event: E) {
        let _ = self.sender.send(event);
    }

    /// Synchronously execute a blocking closure on the runtime's blocking
    /// thread pool, blocking the calling thread until complete.
    pub fn block_on<F, T>(&self, f: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.runtime.spawn_blocking(move || {
            let _ = tx.send(f());
        });
        rx.blocking_recv().expect("Pool::block_on: task panicked")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn pool() -> (Pool<u32>, mpsc::UnboundedReceiver<u32>) {
        let runtime = Arc::new(Runtime::new().expect("tokio runtime"));
        let (tx, rx) = mpsc::unbounded_channel();
        (Pool::new(runtime, tx), rx)
    }

    #[test]
    fn send_event_delivers_the_value_unmodified() {
        let (pool, mut rx) = pool();
        pool.send_event(7);
        assert_eq!(rx.blocking_recv(), Some(7));
    }

    #[test]
    fn clones_share_one_event_channel() {
        let (pool, mut rx) = pool();
        let clone = pool.clone();
        clone.send_event(1);
        pool.send_event(2);
        assert_eq!(rx.blocking_recv(), Some(1));
        assert_eq!(rx.blocking_recv(), Some(2));
    }

    #[test]
    fn block_on_runs_the_closure_and_returns_its_value() {
        let (pool, _rx) = pool();
        assert_eq!(pool.block_on(|| 41 + 1), 42);
    }

    #[test]
    fn spawn_blocking_runs_the_job() {
        let (pool, mut rx) = pool();
        let sender = pool.sender.clone();
        pool.spawn_blocking(move || {
            let _ = sender.send(9);
        });
        assert_eq!(rx.blocking_recv(), Some(9));
    }

    #[test]
    fn spawn_runs_the_future() {
        let (pool, mut rx) = pool();
        let sender = pool.sender.clone();
        pool.spawn(async move {
            let _ = sender.send(3);
        });
        assert_eq!(rx.blocking_recv(), Some(3));
    }

    #[test]
    fn laned_jobs_run_and_report_progress() {
        let (pool, mut rx) = pool();
        let sender = pool.sender.clone();
        let done = Arc::new(AtomicUsize::new(0));
        let counter = done.clone();
        pool.spawn_on(Lane::Foreground, None, move || {
            let _ = sender.send(5);
            counter.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(rx.blocking_recv(), Some(5));
        while done.load(Ordering::SeqCst) == 0 {
            std::thread::yield_now();
        }
    }
}
