//! Scheduled tree-sitter parsing — keeps per-buffer trees fresh on the
//! lane scheduler so highlight queries reuse them instead of parsing
//! inline on the interactive path.
//!
//! Ownership: parsed trees cross from the lane job to the foreground
//! through a bounded handoff store (8 entries, oldest dropped); the
//! `TsTreeReady` event tells the foreground to pick up any entry whose
//! revision still matches its buffer.  Stale trees are dropped, never
//! applied (rework.md §2).
//!
//! The parse reads an O(1) rope snapshot (ropey shares nodes on clone) —
//! never a whole-document String, and never under the buffer lock.  When
//! the cached tree's base plus exactly one pending edit is available, the
//! reparse is incremental; everything else reparses from scratch.

use std::collections::VecDeque;
use std::sync::LazyLock;

use super::ts_registry::TsTreeState;

const HANDOFF_CAP: usize = 8;

static HANDOFF: LazyLock<std::sync::Mutex<VecDeque<(usize, u64, u64, tree_sitter::Tree)>>> =
    LazyLock::new(|| std::sync::Mutex::new(VecDeque::new()));

fn point_of(buf: &crate::text_engine::Buffer, offset: usize) -> tree_sitter::Point {
    let (row, col) = buf.line_col_bytes(offset);
    tree_sitter::Point::new(row, col)
}

/// The tree-sitter edit for a single-edit transaction, with honest points:
/// bytes before the edit's start are untouched in the current text, so its
/// start point reads straight off the buffer; the old end advances through
/// the displaced text itself; the new end reads off the buffer.
fn single_edit_input_edit(
    buf: &crate::text_engine::Buffer,
    start: usize,
    old_end: usize,
    inserted_len: usize,
    old_text: &str,
) -> tree_sitter::InputEdit {
    let start_point = point_of(buf, start);
    let old_end_point = if old_text.is_empty() {
        start_point
    } else {
        let rows = old_text.bytes().filter(|&b| b == b'\n').count();
        match old_text.rfind('\n') {
            Some(i) => tree_sitter::Point::new(
                start_point.row + rows,
                old_text.len() - i - 1,
            ),
            None => tree_sitter::Point::new(
                start_point.row,
                start_point.column + old_text.len(),
            ),
        }
    };
    let new_end_point = if inserted_len == 0 {
        start_point
    } else {
        point_of(buf, start + inserted_len)
    };
    tree_sitter::InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: start + inserted_len,
        start_position: start_point,
        old_end_position: old_end_point,
        new_end_position: new_end_point,
    }
}

/// Schedule a parse for `buf_key` if a grammar is registered for it.
pub fn schedule_tree_update(ed: &mut crate::state::Editor, buf_key: usize) {
    let Some(lang) = ed.ts_languages.get(&buf_key).cloned() else {
        return;
    };
    let Some(arc) = ed.buffers.get(buf_key).cloned() else {
        return;
    };
    let Some(bg) = ed.background.clone() else {
        return;
    };
    let sender = bg.sender.clone();
    // Snapshot under the lock, parse outside it: a rope clone shares the
    // rope's nodes (O(1)), so the old whole-document copy — taken under the
    // buffer mutex, per keystroke batch — is gone.  The incremental base is
    // captured here because `txns_since` and the line index need the live
    // buffer.
    let (revision, txn_seq, rope, base) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        let revision = buf.revision();
        let txn_seq = buf.txn_seq();
        let rope = buf.rope_handle().clone();
        let base = ed.ts_trees.get(&buf_key).and_then(|state| {
            (state.revision <= revision).then_some(state).and_then(|state| {
                let txns = buf.txns_since(state.txn_seq);
                if txns.len() != 1 || txns[0].edits.len() != 1 {
                    return None;
                }
                let ce = &txns[0].edits[0];
                let edit = single_edit_input_edit(
                    &buf,
                    ce.edit.start(),
                    ce.edit.end(),
                    ce.edit.inserted_len(),
                    &ce.old_text,
                );
                Some((state.tree.clone(), edit))
            })
        });
        (revision, txn_seq, rope, base)
    };
    let coalesce = format!("ts-{buf_key}");
    // Lane P1, coalesced per buffer: typing past a queued parse
    // supersedes it.
    bg.spawn_on(
        crate::runtime::Lane::Foreground,
        Some(&coalesce),
        move || {
            let tree = match base {
                Some((mut old_tree, edit)) => {
                    old_tree.edit(&edit);
                    super::ts_registry::parse_rope(&lang, &rope, Some(&old_tree))
                }
                None => super::ts_registry::parse_rope(&lang, &rope, None),
            };
            let Some(tree) = tree else {
                return;
            };
            {
                let mut handoff = HANDOFF.lock().unwrap_or_else(|e| e.into_inner());
                handoff.push_back((buf_key, revision, txn_seq, tree));
                while handoff.len() > HANDOFF_CAP {
                    handoff.pop_front();
                }
            }
            logging::LogErr::log_warn(sender.send(crate::runtime::BackgroundEvent::TsTreeReady { buf_key, revision }));
        },
    );
}

/// Move any parsed tree matching a live buffer revision into the
/// editor's tree cache.  A `ts-tree-updated` event follows so Janet can
/// re-highlight from the fresh tree — once per completed parse batch,
/// never per keystroke (the buffer-changed handler must not re-highlight
/// synchronously; that was the TAB/typing stall).
pub fn take_ready_tree(ed: &mut crate::state::Editor, buf_key: usize, revision: u64) {
    let mut handoff = HANDOFF.lock().unwrap_or_else(|e| e.into_inner());
    let current = ed
        .buffers
        .get(buf_key)
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision());
    let mut picked_tree = false;
    let mut kept = VecDeque::new();
    while let Some((k, rev, seq, tree)) = handoff.pop_front() {
        if k == buf_key && rev == revision && current == Some(revision) {
            ed.ts_trees.insert(buf_key, TsTreeState { tree, revision, txn_seq: seq });
            picked_tree = true;
        } else {
            kept.push_back((k, rev, seq, tree));
        }
    }
    *handoff = kept;
    if picked_tree {
        let mut data = std::collections::HashMap::new();
        data.insert("buffer-id".to_string(), buf_key.to_string());
        ed.events.emit("ts-tree-updated", data);
    }
}
