//! The project file walk — the background scan that fills the file index.
//!
//! The walk runs on the blocking pool and delivers its result in batches so
//! the project tree fills in incrementally instead of appearing in one huge
//! rebuild when the whole project has been read.  Each batch carries the
//! accumulated list so far, so the foreground applies it statelessly.

use std::collections::HashSet;

use crate::runtime::BackgroundEvent;
use crate::state::Editor;

/// Recursion depth cap: a pathological tree (or a bug in the symlink
/// guard) must never overflow the stack mid-walk.
const MAX_WALK_DEPTH: usize = 128;

/// Files per delivered walk batch.
const BATCH_SIZE: usize = 512;

/// Directories the index never descends into: dot-dirs (VCS, tool state),
/// build output, and dependency trees.
fn index_ignored_dir(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules" || name == "target"
}

/// Does this watcher event change the project's file list?
///
/// Churn inside an ignored directory never lands in the index, and a data
/// write to an already-indexed file doesn't change the list — only creates,
/// removals, and the first sighting of an unknown path do.
pub fn index_change_marks_dirty(
    root: &std::path::Path,
    path: &str,
    kind: &str,
    indexed: &[String],
) -> bool {
    // The editor's own atomic-write temp files are never index entries —
    // counting them would re-walk the whole project on every save.
    if std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().contains(".magma-tmp-"))
        .unwrap_or(false)
    {
        return false;
    }
    let Ok(rel_path) = std::path::Path::new(path).strip_prefix(root) else {
        return false;
    };
    let mut components = rel_path.components().peekable();
    while let Some(component) = components.next() {
        if components.peek().is_some()
            && index_ignored_dir(component.as_os_str().to_string_lossy().as_ref())
        {
            return false;
        }
    }
    match kind {
        "removed" => true,
        "modified" => !indexed.contains(&rel_path.to_string_lossy().into_owned()),
        _ => true,
    }
}

/// Sends the walk's accumulated state as `index::Event::ProjectIndexed`
/// batches, ordered by the event channel.
struct Batcher {
    project_name: String,
    generation: u64,
    sender: tokio::sync::mpsc::UnboundedSender<BackgroundEvent>,
    files: Vec<String>,
    ignored: HashSet<String>,
    total: usize,
}

impl Batcher {
    fn push_file(&mut self, rel: String) {
        self.files.push(rel);
        self.total += 1;
        if self.files.len() >= BATCH_SIZE {
            self.flush(false);
        }
    }

    fn flush(&mut self, done: bool) {
        let event = BackgroundEvent::Index(crate::index::Event::ProjectIndexed {
            project_name: self.project_name.clone(),
            generation: self.generation,
            files: self.files.clone(),
            ignored_files: self.ignored.iter().cloned().collect(),
            done,
        });
        let _ = self.sender.send(event);
        self.files.clear();
    }
}

/// Recursively list files under `dir`, skipping VCS/build/dependency dirs.
/// Paths are stored **relative to `root`** — the project's canonical file
/// identity everywhere (sidebar tree, quick-open, project graph).  An
/// unreadable directory or entry is skipped, never fatal: one permission
/// error must not blank the whole index.
fn walk_inner(
    dir: &std::path::Path,
    root: &std::path::Path,
    stack: &mut super::gitignore::IgnoreStack,
    batch: &mut Batcher,
    depth: usize,
) {
    if depth > MAX_WALK_DEPTH {
        return;
    }
    let pushed_ignore = super::gitignore::parse_file(&dir.join(".gitignore"))
        .map(|gi| {
            stack.push(gi);
            true
        })
        .unwrap_or(false);
    let read_dir = std::fs::read_dir(dir);
    if let Ok(entries) = read_dir {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let is_dir = ft.is_dir();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            // Hardcoded ignored dirs (dot, node_modules, target): skip entirely.
            if is_dir && index_ignored_dir(&name) {
                continue;
            }
            let gitignored = stack.ignored(&path, is_dir);
            let rel = path
                .strip_prefix(root)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string());
            if gitignored && let Some(r) = &rel {
                batch.ignored.insert(r.clone());
            }
            if is_dir {
                if !gitignored {
                    walk_inner(&path, root, stack, batch, depth + 1);
                }
            } else if let Some(r) = rel
                && !gitignored
            {
                batch.push_file(r);
            }
        }
    }
    if pushed_ignore {
        stack.pop();
    }
}

/// Enqueue an async walk of the active project's files.  Results arrive as
/// batched `BackgroundEvent::Index(index::Event::ProjectIndexed)` events
/// stamped with the project's generation; the dirty flag clears at enqueue
/// time so a slow walk never queues duplicates.
pub fn spawn_file_index(ed: &mut Editor) -> bool {
    let Some(root) = ed.project_manager.project.root.clone() else {
        return false;
    };
    let Some(bg) = ed.background.clone() else {
        return false;
    };
    let name = ed.project_manager.project.name.clone().unwrap_or_default();
    let generation = ed.project_manager.project.generation;
    let sender = bg.sender.clone();
    ed.project_manager.project.file_index_dirty = false;
    ed.project_manager.project.indexing = true;
    bg.spawn_blocking(move || {
        let started = std::time::Instant::now();
        let mut batch = Batcher {
            project_name: name,
            generation,
            sender,
            files: Vec::new(),
            ignored: HashSet::new(),
            total: 0,
        };
        let mut stack = super::gitignore::IgnoreStack::default();
        walk_inner(&root, &root, &mut stack, &mut batch, 0);
        info!(
            "index: walked '{}' — {} files in {:?}",
            root.display(),
            batch.total,
            started.elapsed()
        );
        batch.flush(true);
    });
    true
}

/// Test entry point: the index walk over `root`.
pub fn walk_files_for_test(root: &std::path::Path) -> Vec<String> {
    let (sender, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut batch = Batcher {
        project_name: String::new(),
        generation: 0,
        sender,
        files: Vec::new(),
        ignored: HashSet::new(),
        total: 0,
    };
    let mut stack = super::gitignore::IgnoreStack::default();
    walk_inner(root, root, &mut stack, &mut batch, 0);
    std::mem::take(&mut batch.files)
}
