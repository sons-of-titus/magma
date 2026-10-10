//! Scheduled tree-sitter parsing — keeps per-buffer trees fresh on the
//! lane scheduler so highlight queries reuse them instead of parsing
//! inline on the interactive path.
//!
//! Ownership: parsed trees cross from the lane job to the foreground
//! through a bounded handoff store (8 entries, oldest dropped); the
//! `TsTreeReady` event tells the foreground to pick up any entry whose
//! revision still matches its buffer.  Stale trees are dropped, never
//! applied (rework.md §2).

use std::collections::VecDeque;
use std::sync::LazyLock;

use super::ts_registry::TsTreeState;

const HANDOFF_CAP: usize = 8;

static HANDOFF: LazyLock<std::sync::Mutex<VecDeque<(usize, u64, tree_sitter::Tree)>>> =
    LazyLock::new(|| std::sync::Mutex::new(VecDeque::new()));

/// Schedule a parse for `buf_key` if a grammar is registered for it.
pub fn schedule_tree_update(ed: &mut crate::kernel::state::Editor, buf_key: usize) {
    let Some(lang) = ed.ts_languages.get(&buf_key).cloned() else { return };
    let Some(arc) = ed.buffers.get(buf_key).cloned() else { return };
    let Some(bg) = ed.background.clone() else { return };
    let revision = arc.lock().unwrap().revision();
    let coalesce = format!("ts-{buf_key}");
    let sender = bg.sender.clone();
    // Lane P1, coalesced per buffer: typing past a queued parse
    // supersedes it.
    bg.spawn_on(crate::kernel::runtime::Lane::Foreground, Some(&coalesce), move || {
        let (text, still_current) = {
            let Ok(buf) = arc.lock() else { return };
            if buf.revision() != revision {
                (String::new(), false)
            } else {
                (buf.slice(0, buf.len()), true)
            }
        };
        if !still_current { return; }
        let Some(tree) = super::ts_registry::parse_tree(&lang, &text) else { return };
        {
            let mut handoff = HANDOFF.lock().unwrap_or_else(|e| e.into_inner());
            handoff.push_back((buf_key, revision, tree));
            while handoff.len() > HANDOFF_CAP {
                handoff.pop_front();
            }
        }
        let _ = sender.send(crate::kernel::runtime::BackgroundEvent::TsTreeReady {
            buf_key, revision,
        });
    });
}

/// Move any parsed tree matching a live buffer revision into the
/// editor's tree cache.
pub fn take_ready_tree(ed: &mut crate::kernel::state::Editor, buf_key: usize, revision: u64) {
    let mut handoff = HANDOFF.lock().unwrap_or_else(|e| e.into_inner());
    let current = ed.buffers.get(buf_key).map(|a| a.lock().unwrap().revision());
    let mut picked: Vec<(usize, VecDeque<u64>)> = Vec::new();
    let _ = &mut picked;
    let mut kept = VecDeque::new();
    while let Some((k, rev, tree)) = handoff.pop_front() {
        if k == buf_key && rev == revision && current == Some(revision) {
            ed.ts_trees.insert(buf_key, TsTreeState { tree, revision });
        } else {
            kept.push_back((k, rev, tree));
        }
    }
    *handoff = kept;
}
