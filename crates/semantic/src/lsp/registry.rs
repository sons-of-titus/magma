//! Registry of running language servers, owned by the embedding application.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::LspHandle;

/// How many times a language may crash (or fail its handshake) before the
/// kernel stops announcing it to the attach layer — a crash-looping server
/// must not respawn forever.
const RESTART_BUDGET: u32 = 3;

#[derive(Default)]
pub struct LspRegistry {
    servers: HashMap<String, Arc<LspHandle>>,
    /// Languages whose spawn/handshake is in flight.  Set synchronously at
    /// start, cleared only when `Started` lands — the handle check alone
    /// cannot see a server that is still negotiating, so without this a
    /// buffer-created and a project-opened attach race into two processes.
    starting: HashSet<String>,
    /// Restart attempts per language — reset when a server reaches Ready.
    restarts: HashMap<String, u32>,
    /// Buffers with an attached server (buffer slab key → language id).
    /// Recorded at didOpen; drives Rust-side completion/hover requests.
    buffer_languages: HashMap<usize, String>,
}

impl LspRegistry {
    pub fn get(&self, language_id: &str) -> Option<&Arc<LspHandle>> {
        self.servers.get(language_id)
    }

    pub fn note_buffer_language(&mut self, buf_key: usize, language_id: &str) {
        self.buffer_languages
            .insert(buf_key, language_id.to_string());
    }

    pub fn language_for_buffer(&self, buf_key: usize) -> Option<String> {
        self.buffer_languages.get(&buf_key).cloned()
    }

    pub fn insert(&mut self, language_id: &str, handle: Arc<LspHandle>) {
        self.servers.insert(language_id.to_string(), handle);
        self.starting.remove(language_id);
    }

    /// Alias `from`'s live server under `to` as well, so one process
    /// serves several language ids (tsserver for js/ts, clangd for c/cpp).
    /// False when `from` has no live handle yet.
    pub fn share(&mut self, from: &str, to: &str) -> bool {
        match self.servers.get(from).cloned() {
            Some(handle) => {
                self.insert(to, handle);
                true
            }
            None => false,
        }
    }

    /// Reserve a spawn slot: true when a start for this language is not
    /// already running or live.
    pub fn can_start(&self, language_id: &str) -> bool {
        !self.servers.contains_key(language_id) && !self.starting.contains(language_id)
    }

    pub fn mark_starting(&mut self, language_id: &str) {
        self.starting.insert(language_id.to_string());
    }

    /// Spend one restart-budget unit; false once the budget is exhausted.
    pub fn allow_restart(&mut self, language_id: &str) -> bool {
        let n = self.restarts.entry(language_id.to_string()).or_insert(0);
        if *n >= RESTART_BUDGET {
            return false;
        }
        *n += 1;
        true
    }

    /// Reset the restart budget when a server has been stable.
    pub fn note_ready(&mut self, language_id: &str) {
        self.restarts.remove(language_id);
    }

    pub fn exited(&mut self, language_id: &str) {
        // A shared server dies as one: drop every key aliasing the same
        // handle, or the surviving alias keeps routing to a dead process.
        let dead = self.servers.get(language_id).cloned();
        if let Some(dead) = dead {
            self.servers.retain(|_, h| !Arc::ptr_eq(h, &dead));
        }
        self.starting.remove(language_id);
    }

    pub fn forget_buffer(&mut self, buf_key: usize) {
        self.buffer_languages.remove(&buf_key);
    }

    pub fn running(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }

    pub fn shutdown_all(&mut self) {
        for handle in self.servers.values() {
            handle.shutdown();
        }
        self.servers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_guard_blocks_until_started_lands() {
        let mut reg = LspRegistry::default();
        assert!(reg.can_start("rust"));
        reg.mark_starting("rust");
        assert!(
            !reg.can_start("rust"),
            "an in-flight handshake must not double-spawn"
        );
        reg.exited("rust");
        assert!(reg.can_start("rust"), "a dead spawn frees the slot");
    }

    #[test]
    fn restart_budget_is_bounded_and_reset_on_ready() {
        let mut reg = LspRegistry::default();
        for _ in 0..3 {
            assert!(reg.allow_restart("rust"));
        }
        assert!(
            !reg.allow_restart("rust"),
            "crash loops must stop respawning"
        );
        reg.note_ready("rust");
        assert!(
            reg.allow_restart("rust"),
            "a stable server earns a fresh budget"
        );
    }

    #[test]
    fn forget_buffer_clears_language_record() {
        let mut reg = LspRegistry::default();
        reg.note_buffer_language(7, "rust");
        assert_eq!(reg.language_for_buffer(7).as_deref(), Some("rust"));
        reg.forget_buffer(7);
        assert_eq!(reg.language_for_buffer(7), None);
    }

    // A dead-process handle is fine here: the registry only routes Arcs,
    // it never speaks the protocol through them.
    fn fake_handle(language_id: &str) -> Arc<LspHandle> {
        let (outbound, _rx) = tokio::sync::mpsc::unbounded_channel();
        let (init_tx, _init_rx) = std::sync::mpsc::channel();
        let core = Arc::new(Core {
            outbound,
            pending: std::sync::Mutex::new(HashMap::new()),
            sink: Arc::new(NullSink),
            state: std::sync::Mutex::new(ServerState::Ready),
            next_id: std::sync::atomic::AtomicU64::new(1),
            legend: std::sync::Mutex::new(SemanticLegend::default()),
            sync_kind: std::sync::Mutex::new(None),
            init_signal: std::sync::Mutex::new(init_tx),
        });
        Arc::new(LspHandle {
            core,
            language_id: language_id.to_string(),
            child: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    struct NullSink;
    impl EventSink for NullSink {
        fn send(&self, _event: Event) {}
    }

    use super::super::{Core, Event, EventSink, LspHandle, SemanticLegend, ServerState};

    #[test]
    fn share_aliases_one_process_under_two_languages() {
        let mut reg = LspRegistry::default();
        reg.insert("typescript", fake_handle("typescript"));
        assert!(reg.share("typescript", "javascript"));
        assert!(
            !reg.can_start("javascript"),
            "a shared server must not double-spawn"
        );
        assert!(
            reg.get("javascript").is_some(),
            "didOpen routes through the alias"
        );
        assert!(
            !reg.share("kotlin", "scala"),
            "no live handle under `from`, no share"
        );
    }

    #[test]
    fn exited_drops_every_alias_of_the_shared_handle() {
        let mut reg = LspRegistry::default();
        reg.insert("typescript", fake_handle("typescript"));
        reg.share("typescript", "javascript");
        reg.exited("typescript");
        assert!(
            reg.get("javascript").is_none(),
            "a crash kills the whole group, not just one alias"
        );
        assert!(reg.can_start("javascript"));
    }
}
