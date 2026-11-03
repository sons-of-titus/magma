//! Phase 2–5 verification (docs/core-redesign.md): transaction undo,
//! explicit grouping, cursor restoration, revision-based save state,
//! mark correctness through undo, atomic writes, split-window view
//! independence, and multi-cursor single transactions.

use crate::kernel::command::{self, builtin};
use crate::kernel::state::Editor;
use crate::kernel::storage::disk::DiskFileSystem;
use crate::tests::helpers;

fn editor_with(content: &str) -> (Editor, usize) {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let key = ed.create_buffer_from_str("test", content);
    if let Some(win) = ed.view_tree.focused_window_mut() {
        win.buffer_id = Some(key);
    }
    (ed, key)
}

fn buffer_of(ed: &Editor, key: usize) -> String {
    ed.buffers
        .get(key)
        .unwrap()
        .lock()
        .unwrap()
        .slice(0, usize::MAX)
}

// ── Undo grouping and cursor restoration (§4) ──────────────────────────────

#[test]
fn discrete_edits_are_separate_undo_steps() {
    let (mut ed, key) = editor_with("");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(0, "a");
    view.insert(1, "b");
    view.insert(2, "c");
    assert_eq!(buffer_of(&ed, key), "abc");

    let view = ed.views.get_mut(&key).unwrap();
    assert!(view.undo());
    let cursor_after_undo = view.cursor_offset();
    assert_eq!(
        buffer_of(&ed, key),
        "ab",
        "one undo = one discrete edit (no merge window)"
    );
    assert_eq!(cursor_after_undo, 2, "undo restores the edit position");
}

#[test]
fn insert_sessions_form_one_undo_step() {
    let (mut ed, key) = editor_with("");
    {
        let view = ed.views.get_mut(&key).unwrap();
        view.open_undo_session();
    }
    for ch in ["h", "i", "!"] {
        let view = ed.views.get_mut(&key).unwrap();
        let cur = view.cursor_offset();
        view.insert(cur, ch);
    }
    {
        let view = ed.views.get_mut(&key).unwrap();
        view.close_undo_session();
    }
    assert_eq!(buffer_of(&ed, key), "hi!");

    let undone_cursor = {
        let view = ed.views.get_mut(&key).unwrap();
        assert!(view.undo());
        view.cursor_offset()
    };
    let undone_text = buffer_of(&ed, key);
    assert_eq!(undone_text, "", "a whole insert session undoes at once");
    assert_eq!(undone_cursor, 0, "session undo returns to session start");
    let redone_cursor = {
        let view = ed.views.get_mut(&key).unwrap();
        assert!(view.redo());
        view.cursor_offset()
    };
    let redone_text = buffer_of(&ed, key);
    assert_eq!(redone_text, "hi!");
    assert_eq!(redone_cursor, 3, "redo restores the end-of-group position");
}

#[test]
fn undo_redo_branch_is_discarded_explicitly() {
    let (mut ed, key) = editor_with("");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(0, "one ");
    view.insert(4, "two");
    assert!(view.undo());

    let view = ed.views.get_mut(&key).unwrap();
    view.insert(0, "X"); // new edit: redo branch ("two") is gone by policy
    assert!(
        !ed.views
            .get(&key)
            .unwrap()
            .buffer
            .lock()
            .unwrap()
            .can_redo()
    );
    assert_eq!(buffer_of(&ed, key), "Xone ");
}

#[test]
fn undo_and_redo_advance_revisions() {
    let (mut ed, key) = editor_with("");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(0, "abc");
    let after_edit = ed.buffers.get(key).unwrap().lock().unwrap().revision();
    assert_eq!(after_edit, 1);

    let view = ed.views.get_mut(&key).unwrap();
    assert!(view.undo());
    assert_eq!(ed.buffers.get(key).unwrap().lock().unwrap().revision(), 2);

    let view = ed.views.get_mut(&key).unwrap();
    assert!(view.redo());
    assert_eq!(ed.buffers.get(key).unwrap().lock().unwrap().revision(), 3);
}

// ── Marks through undo (the proven-wrong restore, §2.3) ────────────────────

#[test]
fn marks_are_correct_after_edits_and_undo() {
    let (mut ed, key) = editor_with("hello world");
    let mark = {
        let mut buf = ed.buffers.get(key).unwrap().lock().unwrap();
        buf.create_mark(6, true) // start of "world"
    };
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(0, ">> ");
    assert_eq!(
        ed.buffers.get(key).unwrap().lock().unwrap().mark_pos(mark),
        Some(9)
    );

    let view = ed.views.get_mut(&key).unwrap();
    assert!(view.undo());
    assert_eq!(
        ed.buffers.get(key).unwrap().lock().unwrap().mark_pos(mark),
        Some(6),
        "undo transforms the mark back exactly — no stale snapshots"
    );
}

#[test]
fn non_sticky_marks_stay_left_of_insertions() {
    let (mut ed, key) = editor_with("abcdef");
    let sticky = {
        let mut buf = ed.buffers.get(key).unwrap().lock().unwrap();
        buf.create_mark(3, true)
    };
    let fixed = {
        let mut buf = ed.buffers.get(key).unwrap().lock().unwrap();
        buf.create_mark(3, false)
    };
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(3, "XY");
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert_eq!(buf.mark_pos(sticky), Some(5));
    assert_eq!(buf.mark_pos(fixed), Some(3));
}

// ── Save state (§5) ────────────────────────────────────────────────────────

#[test]
fn modified_is_revision_based() {
    let (mut ed, key) = editor_with("content");
    assert!(
        !ed.buffers.get(key).unwrap().lock().unwrap().modified(),
        "loaded content is the saved baseline"
    );

    ed.buffers.get(key).unwrap().lock().unwrap().mark_saved();
    assert!(!ed.buffers.get(key).unwrap().lock().unwrap().modified());
    assert!(!ed.buffers.get(key).unwrap().lock().unwrap().modified());

    let view = ed.views.get_mut(&key).unwrap();
    view.insert(7, "!");
    assert!(ed.buffers.get(key).unwrap().lock().unwrap().modified());

    let view = ed.views.get_mut(&key).unwrap();
    assert!(view.undo());
    assert!(
        !ed.buffers.get(key).unwrap().lock().unwrap().modified(),
        "returning to the saved revision is clean"
    );
}

#[test]
fn stale_save_completion_cannot_clear_modified() {
    // Snapshot at revision N, edit (N+1), then the async write of N completes.
    let (mut ed, key) = editor_with("one");
    let (snapshot_rev, snapshot) = {
        let buf = ed.buffers.get(key).unwrap().lock().unwrap();
        (buf.revision(), buf.slice(0, buf.len()))
    };
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(3, " two");
    assert!(ed.buffers.get(key).unwrap().lock().unwrap().modified());

    // The completion marks only the revision it wrote.
    ed.buffers
        .get(key)
        .unwrap()
        .lock()
        .unwrap()
        .mark_saved_at(snapshot_rev);
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert!(buf.modified(), "edits after the snapshot remain unsaved");
    assert_eq!(buf.slice(0, buf.len()), "one two");
    assert_eq!(snapshot, "one");
}

#[test]
fn atomic_writes_leave_no_torn_or_temp_files() {
    let dir = std::env::temp_dir().join(format!("magma-atomic-{}", std::process::id()));
    let path = dir.join("file.txt");
    let path_str = path.to_str().unwrap().to_string();
    crate::kernel::storage::disk::atomic_write(&path_str, "first").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
    crate::kernel::storage::disk::atomic_write(&path_str, "second write").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "second write");
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .filter(|n| n.contains("magma-tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "temp files must be renamed away: {leftovers:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Split-window view independence (§3) ────────────────────────────────────

#[test]
fn split_windows_have_independent_cursors() {
    let (mut ed, key) = editor_with("line one\nline two\n");
    command::execute_command(&mut ed, "window-split", &Default::default()).unwrap();

    // The new (focused) window forked the view; move its cursor.
    let focused = ed.view_tree.focused_window().unwrap();
    assert!(
        ed.window_views.contains_key(&(focused, key)),
        "split must fork a view"
    );
    let forked_cursor_before = ed
        .window_views
        .get(&(focused, key))
        .unwrap()
        .cursor_offset();

    let view = ed.focused_view_mut().unwrap();
    view.set_cursor(9);

    let fork = ed.window_views.get(&(focused, key)).unwrap();
    assert_eq!(fork.cursor_offset(), 9);
    assert_ne!(fork.cursor_offset(), forked_cursor_before);
    assert_eq!(
        ed.views.get(&key).unwrap().cursor_offset(),
        forked_cursor_before,
        "the other window's cursor is untouched"
    );

    // Both windows still share the document.
    let view = ed.focused_view_mut().unwrap();
    view.insert(9, ">");
    assert_eq!(buffer_of(&ed, key), "line one\n>line two\n");
}

// ── Multi-cursor single transactions (§1.2 / DoD 3) ────────────────────────

#[test]
fn multi_cursor_insert_is_one_transaction_one_revision() {
    let (mut ed, key) = editor_with("aa\nbb\ncc\n");
    let rev_before = ed.buffers.get(key).unwrap().lock().unwrap().revision();
    let view = ed.views.get_mut(&key).unwrap();
    let txn = view.insert_many(&[(0, "1"), (3, "2"), (6, "3")]).unwrap();
    assert_eq!(txn.edits.len(), 3, "three cursors, one transaction");
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    assert_eq!(buf.revision(), rev_before + 1);
    assert_eq!(buf.slice(0, buf.len()), "1aa\n2bb\n3cc\n");
}

// ── Canonical change events (§6) ───────────────────────────────────────────

#[test]
fn typing_queues_a_pending_change_until_publication() {
    let (mut ed, key) = editor_with("hi");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(2, "!");
    assert!(
        ed.buffers
            .get(key)
            .unwrap()
            .lock()
            .unwrap()
            .has_pending_changes()
    );
    ed.publish_buffer_changes();
    assert!(
        !ed.buffers
            .get(key)
            .unwrap()
            .lock()
            .unwrap()
            .has_pending_changes(),
        "publication drains the buffer's pending transactions"
    );
}

#[test]
fn change_event_carries_revision_and_edits() {
    let (mut ed, key) = editor_with("hello");
    let view = ed.views.get_mut(&key).unwrap();
    view.insert(5, " world");
    ed.publish_buffer_changes();
    ed.events.drain_and_dispatch();

    // The pending queue is empty; verify the payload shape through the
    // Janet-facing accessor instead (documented contract of changes-since).
    let changes = ed
        .buffers
        .get(key)
        .unwrap()
        .lock()
        .unwrap()
        .changes_since(0)
        .expect("revision 0 is always retained");
    assert_eq!(changes.len(), 1);
    let (before, after, edits) = &changes[0];
    assert_eq!(*before, 0);
    assert_eq!(*after, 1);
    assert_eq!(edits, &vec![(5usize, 5usize, 11usize)]);

    let view = ed.views.get_mut(&key).unwrap();
    view.delete(0, 6);
    let changes = ed
        .buffers
        .get(key)
        .unwrap()
        .lock()
        .unwrap()
        .changes_since(1)
        .expect("recent revisions are retained");
    assert_eq!(changes.len(), 1);
    let (_, after, edits) = &changes[0];
    assert_eq!(*after, 2);
    assert_eq!(edits, &vec![(0usize, 6usize, 0usize)]);
}

// ── Cursor math uses the line index (§3.3) ─────────────────────────────────

#[test]
fn position_of_handles_multibyte_and_lines() {
    let (ed, key) = editor_with("héllo\nwörld\nx");
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    // line 0: h=0, é=1..3, l=3, l=4, o=5, \n=6; line 1 starts at 7.
    let p = buf.position_of(7); // 'w'
    assert_eq!((p.line, p.column), (1, 0));
    let p = buf.position_of(8); // start of 'ö'
    assert_eq!((p.line, p.column), (1, 1));
    let p = buf.position_of(buf.len()); // end of "x"
    assert_eq!((p.line, p.column), (2, 1));
    drop(buf);
    assert_eq!(helpers::buf_text(&ed), "héllo\nwörld\nx");
}

// ── Editor-level positions ride the transaction stream ─────────────────────

#[test]
fn insert_many_snaps_mid_char_offsets_instead_of_dropping() {
    let (mut ed, key) = editor_with("héllo");
    let view = ed.views.get_mut(&key).unwrap();
    // Offset 2 is inside the two-byte 'é'; offset 6 is the end boundary.
    let txn = view.insert_many(&[(2, "X"), (6, "Y")]);
    assert!(
        txn.is_some(),
        "a mid-char offset must snap to a boundary, not drop the transaction"
    );
    let buf = ed.buffers.get(key).unwrap().lock().unwrap();
    // snap_offset floors: the insert lands before the 'é', like the single
    // insert path, instead of the whole transaction being dropped.
    assert_eq!(buf.slice(0, buf.len()), "hXélloY");
}

fn register_prefix_inserter(ed: &mut Editor) {
    ed.commands.register_fn(
        "test-insert-prefix",
        "test-only: insert XX at offset 0",
        vec![],
        |ed, _args| {
            let view = ed.focused_view_mut().unwrap();
            view.insert(0, "XX");
            Ok(())
        },
    );
}

#[test]
fn extra_cursors_transform_through_command_edits() {
    let (mut ed, _key) = editor_with("one\ntwo\nthree\n");
    ed.multi_cursor.active = true;
    ed.multi_cursor
        .extra_cursors
        .push(crate::kernel::state::cursor::ExtraCursor {
            pos: 4,
            anchor: None,
        });
    register_prefix_inserter(&mut ed);

    command::execute_command(&mut ed, "test-insert-prefix", &std::collections::HashMap::new())
        .unwrap();
    assert_eq!(
        ed.multi_cursor.extra_cursors[0].pos,
        6,
        "an extra cursor after the edit shifts by the inserted length"
    );
}

#[test]
fn extra_cursors_transform_through_undo() {
    let (mut ed, _key) = editor_with("one\ntwo\n");
    ed.multi_cursor
        .extra_cursors
        .push(crate::kernel::state::cursor::ExtraCursor {
            pos: 4,
            anchor: None,
        });
    register_prefix_inserter(&mut ed);
    command::execute_command(&mut ed, "test-insert-prefix", &std::collections::HashMap::new())
        .unwrap();
    assert_eq!(ed.multi_cursor.extra_cursors[0].pos, 6);

    command::execute_command(&mut ed, "undo", &std::collections::HashMap::new()).unwrap();
    assert_eq!(
        ed.multi_cursor.extra_cursors[0].pos,
        4,
        "undo commits an inverse transaction, and extras ride it"
    );
}

#[test]
fn selection_anchor_transforms_through_command_edits() {
    let (mut ed, _key) = editor_with("hello world");
    ed.selection = Some(crate::kernel::state::mode::Selection::char(6));
    register_prefix_inserter(&mut ed);

    command::execute_command(&mut ed, "test-insert-prefix", &std::collections::HashMap::new())
        .unwrap();
    assert_eq!(
        ed.selection.as_ref().unwrap().anchor,
        8,
        "the selection anchor stays on the same character"
    );
}
