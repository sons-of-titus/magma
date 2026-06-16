//! Async file watcher — uses the `notify` crate (inotify / FSEvents) to detect
//! on-disk changes and forward them as `BackgroundEvent::FileChanged`.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

use crate::kernel::runtime::BackgroundEvent;

/// Manages file-system watches for open files.
/// Detects external changes and notifies the main thread via background events.
pub struct FileWatcher {
    /// The notify watcher (runs on its own thread).
    _watcher: RecommendedWatcher,
    /// Set of watched paths (canonicalized).
    watched: Arc<Mutex<HashSet<String>>>,
}

impl FileWatcher {
    /// Create a new file watcher. `sender` forwards events to the editor.
    pub fn new(sender: mpsc::UnboundedSender<BackgroundEvent>) -> Result<Self, String> {
        let watched: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
        let watched_clone = watched.clone();

        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let paths: Vec<String> = event
                        .paths
                        .iter()
                        .filter_map(|p| {
                            let canonical = std::fs::canonicalize(p).ok()?;
                            Some(canonical.to_string_lossy().to_string())
                        })
                        .collect();

                    // Only emit if at least one path is being watched
                    let should_emit = {
                        let w = watched_clone.lock().unwrap();
                        paths.iter().any(|p| w.contains(p))
                    };

                    if should_emit {
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                for path_str in paths {
                                    let _ = sender.send(BackgroundEvent::FileChanged {
                                        path: path_str.clone(),
                                        kind: "modified".to_string(),
                                    });
                                }
                            }
                            EventKind::Remove(_) => {
                                for path_str in paths {
                                    let _ = sender.send(BackgroundEvent::FileChanged {
                                        path: path_str.clone(),
                                        kind: "removed".to_string(),
                                    });
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
        })
    }

    /// Start watching a file path. Only watches canonical paths to avoid duplicates.
    pub fn watch(&mut self, path: &str) {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => Path::new(path).to_path_buf(),
        };
        let path_str = canonical.to_string_lossy().to_string();

        {
            let mut w = self.watched.lock().unwrap();
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
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => Path::new(path).to_path_buf(),
        };
        let path_str = canonical.to_string_lossy().to_string();

        let mut w = self.watched.lock().unwrap();
        w.remove(&path_str);
    }

    /// Check if a path is being watched.
    pub fn is_watched(&self, path: &str) -> bool {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => Path::new(path).to_path_buf(),
        };
        let path_str = canonical.to_string_lossy().to_string();
        let w = self.watched.lock().unwrap();
        w.contains(&path_str)
    }
}
