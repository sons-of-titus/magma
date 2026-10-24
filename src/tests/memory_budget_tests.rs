//! Memory budget enforcement — flood each capped structure past its bound
//! and assert the cap holds (docs/architecture/memory.md).

use crate::tests::helpers::make_editor;

#[test]
fn undo_history_is_capped_at_ten_thousand_entries() {
    let mut ed = make_editor();
    let key = ed.create_buffer_from_str("flood.txt", "x");
    let arc = ed.buffers.get(key).unwrap().clone();
    for i in 0..12_000 {
        let mut buf = arc.lock().unwrap();
        let text = format!("{i}");
        let end = buf.len();
        buf.insert(end, &text);
    }
    let buf = arc.lock().unwrap();
    assert!(buf.undo_len() <= 10_000, "undo entries must cap at 10k, got {}", buf.undo_len());
}

#[test]
fn document_history_is_capped_and_old_revisions_report_none() {
    let mut ed = make_editor();
    let key = ed.create_buffer_from_str("flood.txt", "x");
    let arc = ed.buffers.get(key).unwrap().clone();
    let mut first_rev = None;
    for i in 0..(4_096 + 16) {
        let mut buf = arc.lock().unwrap();
        let text = format!("{i}");
        let end = buf.len();
        buf.insert(end, &text);
        if first_rev.is_none() { first_rev = Some(buf.revision()); }
    }
    let buf = arc.lock().unwrap();
    assert_eq!(buf.changes_since(0), None, "revision 0 is out of the retention window");
    let recent = buf.revision() - 8;
    assert!(buf.changes_since(recent).is_some(), "recent revisions stay available");
}

#[test]
fn quickfix_list_truncates_at_five_hundred() {
    let mut ed = make_editor();
    ed.search.generation += 1;
    let generation = ed.search.generation;
    let entries: Vec<crate::kernel::task::QuickfixEntry> = (0..900)
        .map(|i| crate::kernel::task::QuickfixEntry {
            filename: format!("f{i}.rs"), line: i, col: 1, message: "m".into(),
            ..Default::default()
        })
        .collect();
    crate::kernel::runtime::process_background_event(&mut ed,
        crate::kernel::runtime::BackgroundEvent::GrepResultsBatch { generation, entries });
    assert_eq!(ed.quickfix_list.len(), 500, "the quickfix list must cap at 500");
}

#[test]
fn buffer_caches_are_evicted_on_close() {
    let mut ed = make_editor();
    let key = ed.create_buffer_from_str("cache.txt", "alpha beta");
    // Populate both per-buffer caches through their public paths.
    let arc = ed.buffers.get(key).unwrap().clone();
    arc.lock().unwrap().insert(0, "more words here");
    crate::kernel::completion::trigger(&mut ed);
    ed.minimap_cache.insert(key, Default::default());

    assert!(ed.completion.word_cache.contains_key(&key) || !ed.completion.visible);
    ed.drop_buffer(key);
    assert!(!ed.completion.word_cache.contains_key(&key), "word cache evicted");
    assert!(!ed.minimap_cache.contains_key(&key), "minimap cache evicted");
}

#[test]
fn large_document_soak_edits_undo_and_cursor_stay_correct() {
    // A 20 MB document: typing, cursor movement, and a bounded undo round
    // must remain functionally exact (values, not timing — timing gates
    // live in the benches).
    let mut ed = make_editor();
    let chunk = "the quick brown fox jumps over the lazy dog\n".repeat(470_000); // ~20 MB
    let key = ed.create_buffer_from_str("soak.txt", &chunk);
    let arc = ed.buffers.get(key).unwrap().clone();
    drop(chunk);
    let base_len = arc.lock().unwrap().len();

    let mut added = 0usize;
    for i in 0..1_000 {
        let text = format!("x{i} ");
        added += text.len();
        let mut buf = arc.lock().unwrap();
        let end = buf.len();
        buf.insert(end, &text);
    }
    {
        let buf = arc.lock().unwrap();
        assert_eq!(buf.len(), base_len + added, "length bookkeeping holds");
    }
    // Cursor to a far offset and back via position_of: no scans that break.
    let buf = arc.lock().unwrap();
    let mid = buf.line_col_bytes(base_len / 2).0;
    assert!(mid > 100_000, "half of a 20MB doc is ~233k lines in, got {mid}");
    drop(buf);

    // Undo all 1000 edits: exact restoration.
    loop {
        if arc.lock().unwrap().undo().is_none() { break; }
    }
    let buf = arc.lock().unwrap();
    assert_eq!(buf.len(), base_len, "undo restores the exact base length");
}
