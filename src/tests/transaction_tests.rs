//! Transaction normalization, commit semantics, atomicity, inversion.

use crate::kernel::text_engine::document::Document;
use crate::kernel::text_engine::edit::Edit;
use crate::kernel::text_engine::revision::Revision;
use crate::kernel::text_engine::transaction::{
    TransactionBuilder, TransactionError, TransactionOrigin,
};

#[test]
fn normalize_sorts_unsorted_edits() {
    let edits = TransactionBuilder::new()
        .replace(6..8, "x")
        .insert(1, "y")
        .normalize()
        .unwrap();
    assert_eq!(edits.len(), 2);
    assert_eq!((edits[0].start(), edits[1].start()), (1, 6));
}

#[test]
fn normalize_merges_abutting_edits() {
    let edits = TransactionBuilder::new()
        .delete(0..2)
        .delete(2..4)
        .normalize()
        .unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range, 0..4);

    // Insert butting against a delete folds into one replacement.
    let edits = TransactionBuilder::new()
        .insert(2, "ab")
        .delete(2..5)
        .normalize()
        .unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range, 2..5);
    assert_eq!(edits[0].inserted_text, "ab");
}

#[test]
fn normalize_rejects_overlapping_edits() {
    let err = TransactionBuilder::new()
        .delete(0..3)
        .delete(2..5)
        .normalize()
        .unwrap_err();
    assert!(matches!(err, TransactionError::OverlappingEdits { .. }));
}

#[test]
fn normalize_rejects_inverted_ranges() {
    for builder in [
        TransactionBuilder::new().replace(5..3, "x"),
        TransactionBuilder::new().delete(5..3),
    ] {
        assert!(matches!(
            builder.normalize().unwrap_err(),
            TransactionError::InvalidRange(_)
        ));
    }
}

#[test]
fn empty_builder_commits_nothing() {
    let mut doc = Document::from_string("abc");
    assert!(doc.commit(TransactionBuilder::new()).unwrap().is_none());
    assert!(doc
        .commit(TransactionBuilder::new().insert(1, ""))
        .unwrap()
        .is_none());
    assert_eq!(doc.revision(), Revision(0));
    assert_eq!(doc.history().len(), 0);
}

#[test]
fn one_commit_is_exactly_one_revision() {
    let mut doc = Document::new();
    let txn = doc
        .commit(TransactionBuilder::new().insert(0, "hello"))
        .unwrap()
        .unwrap();
    assert_eq!(txn.revision_before, Revision(0));
    assert_eq!(txn.revision_after, Revision(1));
    assert_eq!(doc.revision(), Revision(1));
    assert_eq!(doc.history().len(), 1);
    assert_eq!(doc.changes_since(Revision(0)).map(|c| c.len()), Some(1));
    assert_eq!(doc.changes_since(Revision(1)).map(|c| c.len()), Some(0));
}

#[test]
fn multi_cursor_edits_share_one_revision_transition() {
    let mut doc = Document::from_string("aaa\nbbb\nccc");
    let txn = doc
        .commit(
            TransactionBuilder::new()
                .insert(0, "X")
                .insert(4, "Y")
                .insert(8, "Z"),
        )
        .unwrap()
        .unwrap();
    assert_eq!(doc.text(), "Xaaa\nYbbb\nZccc");
    assert_eq!(txn.revision_before, Revision(0));
    assert_eq!(txn.revision_after, Revision(1));
    assert_eq!(txn.edits.len(), 3);
    assert_eq!(doc.revision(), Revision(1));
}

#[test]
fn builder_origin_is_recorded() {
    let mut doc = Document::new();
    let txn = doc
        .commit(
            TransactionBuilder::new()
                .with_origin(TransactionOrigin::Format)
                .insert(0, "x"),
        )
        .unwrap()
        .unwrap();
    assert_eq!(txn.origin, TransactionOrigin::Format);
}

#[test]
fn commit_is_atomic_on_validation_failure() {
    let mut doc = Document::from_string("abc");
    let result = doc.commit(
        TransactionBuilder::new()
            .insert(0, "X")
            .replace(2..99, "y"),
    );
    assert!(matches!(result, Err(TransactionError::OutOfBounds { .. })));
    assert_eq!(doc.text(), "abc");
    assert_eq!(doc.revision(), Revision(0));
    assert_eq!(doc.history().len(), 0);
}

#[test]
fn commit_rejects_non_char_boundary_offsets() {
    let mut doc = Document::from_string("héllo"); // 'é' occupies bytes 1..3
    assert!(matches!(
        doc.commit(TransactionBuilder::new().insert(2, "x")).unwrap_err(),
        TransactionError::NotCharBoundary { .. }
    ));
    assert!(matches!(
        doc.commit(TransactionBuilder::new().delete(1..2)).unwrap_err(),
        TransactionError::NotCharBoundary { .. }
    ));
    assert_eq!(doc.text(), "héllo");
    assert_eq!(doc.revision(), Revision(0));
}

#[test]
fn committed_old_text_matches_pre_edit_content() {
    let mut doc = Document::from_string("hello world");
    let txn = doc
        .commit(TransactionBuilder::new().replace(4..8, "[]"))
        .unwrap()
        .unwrap();
    assert_eq!(txn.edits[0].old_text, "o wo");
    assert_eq!(doc.text(), "hell[]rld");
}

#[test]
fn inverse_transaction_restores_text_and_bumps_revision() {
    let mut doc = Document::from_string("hello");
    let before = Revision(doc.revision().0);
    let txn = doc
        .commit(
            TransactionBuilder::new()
                .replace(1..3, "EY")
                .insert(5, "!"),
        )
        .unwrap()
        .unwrap();
    assert_eq!(doc.text(), "hEYlo!");

    let undo = doc.commit(txn.invert(TransactionOrigin::Undo)).unwrap().unwrap();
    assert_eq!(doc.text(), "hello");
    assert_eq!(undo.origin, TransactionOrigin::Undo);
    assert_eq!(undo.revision_before, before.next());
    assert_eq!(undo.revision_after, before.next().next());
    assert_eq!(doc.revision(), before.next().next(), "undo never reuses a revision");
}

#[test]
fn revision_chain_is_continuous_across_undo() {
    let mut doc = Document::new();
    doc.commit(TransactionBuilder::new().insert(0, "a")).unwrap().unwrap();
    let txn = doc.commit(TransactionBuilder::new().insert(1, "b")).unwrap().unwrap();
    doc.commit(txn.invert(TransactionOrigin::Undo)).unwrap().unwrap();
    doc.commit(TransactionBuilder::new().insert(1, "c")).unwrap().unwrap();

    let history = doc.history();
    assert_eq!(history.len(), 4);
    for pair in history.windows(2) {
        assert_eq!(pair[0].revision_after, pair[1].revision_before);
    }
    assert_eq!(doc.changes_since(Revision(1)).map(|c| c.len()), Some(3));
    assert_eq!(doc.changes_since(Revision(3)).map(|c| c.len()), Some(1));
}

#[test]
fn edits_at_bof_and_eof() {
    let mut doc = Document::from_string("middle");
    doc.commit(TransactionBuilder::new().insert(0, "start\n")).unwrap().unwrap();
    doc.commit(TransactionBuilder::new().insert(doc.len(), "\nend")).unwrap().unwrap();
    assert_eq!(doc.text(), "start\nmiddle\nend");
    assert_eq!(doc.revision(), Revision(2));
}

#[test]
fn inverse_edits_are_ascending_and_disjoint() {
    let mut doc = Document::from_string("abcdefghij");
    let txn = doc
        .commit(
            TransactionBuilder::new()
                .replace(2..4, "Z")
                .insert(8, "QQ"),
        )
        .unwrap()
        .unwrap();
    let inv = txn.inverse_edits();
    for pair in inv.windows(2) {
        assert!(pair[0].end() <= pair[1].start());
    }
    // Applying the inverse via the builder normalizes without error.
    assert_eq!(inv.len(), txn.invert(TransactionOrigin::Undo).normalize().unwrap().len());
}

#[test]
fn inverse_ranges_shift_by_preceding_edit_deltas() {
    // Regression (caught by the property suite): a preceding insertion moves
    // later edits' inserted text, so their inverse ranges must carry the
    // cumulative delta — not their original pre-transaction offsets.
    let mut doc = Document::from_string("abcdef");
    let txn = doc
        .commit(
            TransactionBuilder::new()
                .insert(2, "XY") // delta +2
                .delete(4..5),   // post-transaction position: 4 + 2
        )
        .unwrap()
        .unwrap();
    assert_eq!(doc.text(), "abXYcdf");
    let inv = txn.inverse_edits();
    assert_eq!(inv[0].range, 2..4); // where "XY" landed
    assert_eq!(inv[1].range, 6..6); // shifted by the preceding +2
    doc.commit(txn.invert(TransactionOrigin::Undo)).unwrap().unwrap();
    assert_eq!(doc.text(), "abcdef");
}

#[test]
fn edit_constructors_encode_their_intent() {
    assert_eq!(Edit::insert(3, "ab").range, 3..3);
    assert!(Edit::insert(3, "ab").is_insert_like());
    assert!(Edit::delete(3..5).is_delete_like());
    assert_eq!(Edit::replace(3..5, "xy").new_range(), 3..5);
    assert!(Edit::insert(3, "").is_noop());
}
