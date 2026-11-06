//! Async file watcher — uses the `notify` crate (inotify / FSEvents) to detect
//! on-disk changes and forward them as `BackgroundEvent::FileChanged`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::runtime::BackgroundEvent;

/// Manages file-system watches for open files.
/// Detects external changes and notifies the main thread via background events.
pub struct FileWatcher {
    /// The notify watcher (runs on its own thread).
    _watcher: RecommendedWatcher,
    /// Set of watched paths (canonicalized).
    watched: Arc<Mutex<HashSet<String>>>,
    /// Project roots watched recursively (canonicalized, no trailing slash).
    project_roots: Arc<Mutex<HashSet<String>>>,
    /// Input path → canonical path.  `maintain` re-checks every open buffer
    /// at the frame-timer cadence; canonicalize is several syscalls per
    /// call, so stable paths must not re-hit the filesystem (profiled at
    /// ~10% of main-thread time while idle).
    canon_cache: Mutex<HashMap<String, PathBuf>>,
}

impl FileWatcher {
    /// Create a new file watcher. `sender` forwards events to the editor.
    pub fn new(sender: mpsc::UnboundedSender<BackgroundEvent>) -> Result<Self, String> {
        let watched: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let watched_clone = watched.clone();
        let project_roots: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let roots_clone = project_roots.clone();

        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let paths: Vec<String> = event
                        .paths
                        .iter()
                        .map(|p| {
                            // A removed path cannot be canonicalized — the
                            // event must survive as the raw path, or external
                            // deletions never reach reload or the index.
                            std::fs::canonicalize(p)
                                .unwrap_or_else(|_| p.clone())
                                .to_string_lossy()
                                .to_string()
                        })
                        .collect();

                    // Only emit if at least one path is being watched
                    let should_emit = {
                        let w = watched_clone.lock().unwrap_or_else(|e| e.into_inner());
                        if paths.iter().any(|p| w.contains(p)) {
                            true
                        } else {
                            let roots = roots_clone.lock().unwrap_or_else(|e| e.into_inner());
                            paths
                                .iter()
                                .any(|p| roots.iter().any(|r| p.starts_with(r.as_str())))
                        }
                    };

                    if should_emit {
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                for path_str in paths {
                                    logging::LogErr::log_warn(sender.send(BackgroundEvent::FileChanged {
                                        path: path_str.clone(),
                                        kind: "modified".to_string(),
                                    }));
                                }
                            }
                            EventKind::Remove(_) => {
                                for path_str in paths {
                                    logging::LogErr::log_warn(sender.send(BackgroundEvent::FileChanged {
                                        path: path_str.clone(),
                                        kind: "removed".to_string(),
                                    }));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| format!("Failed to create file watcher: {e}"))?;

        Ok(FileWatcher {
            _watcher: watcher,
            watched,
            project_roots,
            canon_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Canonicalize `path` through the cache; `None` results (missing files)
    /// are never cached so a recreated path canonicalizes normally.
    fn canonical(&self, path: &str) -> Option<PathBuf> {
        if let Ok(cache) = self.canon_cache.lock()
            && let Some(cached) = cache.get(path)
        {
            return Some(cached.clone());
        }
        let canonical = std::fs::canonicalize(path).ok()?;
        if let Ok(mut cache) = self.canon_cache.lock() {
            cache.insert(path.to_string(), canonical.clone());
        }
        Some(canonical)
    }

    /// Watch a project root recursively: any create/modify/remove underneath
    /// is reported, so the file index can be marked dirty and rescanned.
    /// The error is surfaced to the user — a silently unwatched project
    /// shows a stale tree with no hint why (Linux `max_user_watches`).
    pub fn watch_project(&mut self, root: &str) -> Result<(), String> {
        let canonical = std::fs::canonicalize(root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| root.to_string());
        {
            let mut roots = self.project_roots.lock().unwrap_or_else(|e| e.into_inner());
            if !roots.insert(canonical.clone()) {
                return Ok(());
            }
        }
        self._watcher
            .watch(std::path::Path::new(&canonical), RecursiveMode::Recursive)
            .map_err(|e| format!("watching {canonical:?}: {e}"))
    }

    /// Drop every watched project root except `keep`: a project switch
    /// must not leave the old root's recursive watch feeding events (and
    /// foreground work) forever.
    pub fn retain_project_roots(&mut self, keep: &str) {
        let canonical_keep = std::fs::canonicalize(keep)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| keep.to_string());
        let stale: Vec<String> = {
            let roots = self.project_roots.lock().unwrap_or_else(|e| e.into_inner());
            roots
                .iter()
                .filter(|r| **r != canonical_keep)
                .cloned()
                .collect()
        };
        for r in &stale {
            logging::LogErr::log_err(self._watcher.unwatch(std::path::Path::new(r)));
            let mut roots = self.project_roots.lock().unwrap_or_else(|e| e.into_inner());
            roots.remove(r);
        }
    }

    /// Forget all individually-watched file entries (and their parent-dir
    /// notify watches).  The frame sync re-watches whatever is open next
    /// frame; outgoing-project files must not stay watched after a switch.
    pub fn clear_watched_files(&mut self) {
        let mut w = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        for p in w.drain() {
            let parent = std::path::Path::new(&p).parent().map(Path::to_path_buf);
            if let Some(parent) = parent {
                logging::LogErr::log_err(self._watcher.unwatch(&parent));
            }
        }
    }

    /// Start watching a file path. Only watches canonical paths to avoid duplicates.
    pub fn watch(&mut self, path: &str) {
        let Some(canonical) = self.canonical(path) else {
            return;
        };
        let path_str = canonical.to_string_lossy().to_string();

        {
            let mut w = self.watched.lock().unwrap_or_else(|e| e.into_inner());
            if !w.insert(path_str.clone()) {
                return;
            }
        }

        let parent = canonical.parent().unwrap_or(Path::new("."));
        if let Err(e) = self._watcher.watch(parent, RecursiveMode::NonRecursive) {
            debug!("FileWatcher: failed to watch {parent:?}: {e}");
        }
    }

    /// Stop watching a file path.
    pub fn unwatch(&mut self, path: &str) {
        let Some(canonical) = self.canonical(path) else {
            return;
        };
        let path_str = canonical.to_string_lossy().to_string();

        let mut w = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        w.remove(&path_str);
    }

    /// Check if a path is being watched.
    pub fn is_watched(&self, path: &str) -> bool {
        let Some(canonical) = self.canonical(path) else {
            return false;
        };
        let path_str = canonical.to_string_lossy().to_string();
        let w = self.watched.lock().unwrap_or_else(|e| e.into_inner());
        w.contains(&path_str)
    }
}
