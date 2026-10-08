//! Anchor boundary contract — docs/core-redesign.md §0.4 / §A.2.
//! Every case is hand-computed against a known string.

use crate::kernel::text_engine::anchor::{
    Affinity, Anchor, transform_anchor, transform_anchor_through_edits,
};
use crate::kernel::text_engine::document::Document;
use crate::kernel::text_engine::edit::Edit;
use crate::kernel::text_engine::transaction::TransactionOrigin;

fn at(offset: usize, affinity: Affinity) -> Anchor {
    Anchor { offset, affinity }
}

fn moved(anchor: Anchor, edit: &Edit) -> usize {
    transform_anchor(anchor, edit).offset
}

#[test]
fn insertion_strictly_before_anchor_is_unchanged() {
    let edit = Edit::insert(6, "XY");
    assert_eq!(moved(at(0, Affinity::Before), &edit), 0);
    assert_eq!(moved(at(5, Affinity::After), &edit), 5);
}

#[test]
fn insertion_strictly_after_anchor_shifts_by_len() {
    let edit = Edit::insert(6, "XY");
    assert_eq!(moved(at(7, Affinity::Before), &edit), 9);
    assert_eq!(moved(at(11, Affinity::After), &edit), 13);
}

#[test]
fn insertion_exactly_at_anchor_respects_affinity() {
    let edit = Edit::insert(6, "XY");
    assert_eq!(moved(at(6, Affinity::Before), &edit), 6, "Before stays left");
    assert_eq!(moved(at(6, Affinity::After), &edit), 8, "After rides right");
}

#[test]
fn deletion_containing_anchor_collapses_to_start() {
    let edit = Edit::delete(4..8); // "o wo"
    assert_eq!(moved(at(6, Affinity::Before), &edit), 4);
    assert_eq!(moved(at(6, Affinity::After), &edit), 4);
}

#[test]
fn deletion_starting_at_anchor_collapses_to_start() {
    let edit = Edit::delete(4..8);
    assert_eq!(moved(at(4, Affinity::Before), &edit), 4);
    assert_eq!(moved(at(4, Affinity::After), &edit), 4);
}

#[test]
fn deletion_ending_at_anchor_collapses_to_start() {
    let edit = Edit::delete(4..8);
    // Pure deletion: nothing replaces the span, both affinities land on s.
    assert_eq!(moved(at(8, Affinity::Before), &edit), 4);
    assert_eq!(moved(at(8, Affinity::After), &edit), 4);
}

#[test]
fn replacement_anchor_on_boundaries_lands_on_span_edges() {
    // "hello world" → "hell[]rld": old byte 8 ('r') survives at new offset 6.
    let edit = Edit::replace(4..8, "[]");
    assert_eq!(moved(at(8, Affinity::After), &edit), 6, "rides surviving text");
    assert_eq!(moved(at(8, Affinity::Before), &edit), 4, "hugs replaced text");
    assert_eq!(moved(at(4, Affinity::Before), &edit), 4);
    assert_eq!(moved(at(4, Affinity::After), &edit), 6);
    assert_eq!(moved(at(6, Affinity::After), &edit), 6, "inside deleted span");
}

#[test]
fn replacement_outside_anchor_shifts_by_delta() {
    let edit = Edit::replace(4..8, "[]"); // delta = 2 - 4 = -2
    assert_eq!(moved(at(2, Affinity::Before), &edit), 2);
    assert_eq!(moved(at(11, Affinity::After), &edit), 9);
}

#[test]
fn empty_insertion_is_unchanged_for_both_affinities() {
    let edit = Edit::insert(6, "");
    assert_eq!(moved(at(6, Affinity::Before), &edit), 6);
    assert_eq!(moved(at(6, Affinity::After), &edit), 6);
    assert_eq!(moved(at(11, Affinity::After), &edit), 11);
}

#[test]
fn single_edit_transform_matches_multi_edit_form() {
    let edit = Edit::replace(4..8, "[]");
    for offset in [0usize, 3, 4, 6, 8, 9, 11] {
        for affinity in [Affinity::Before, Affinity::After] {
            let a = at(offset, affinity);
            assert_eq!(
                transform_anchor(a, &edit),
                transform_anchor_through_edits(a, &[edit.clone()]),
                "offset {offset} {affinity:?}"
            );
        }
    }
}

#[test]
fn multi_edit_transaction_transforms_every_region() {
    // Edits (normalized, pre-transaction coordinates): replace [2,4)→"Z"
    // (delta -1), insert at 8 "QQ" (delta +2).  Text "abcdefghij" →
    // "abZefghQQij".
    let edits = vec![Edit::replace(2..4, "Z"), Edit::insert(8, "QQ")];
    let cases: &[(usize, Affinity, usize)] = &[
        (0, Affinity::Before, 0),   // before all
        (3, Affinity::After, 3),    // inside first: s + L = 2 + 1
        (6, Affinity::Before, 5),   // gap between edits: 6 + (-1)
        (8, Affinity::After, 9),   // on second edit's start: 8 - 1 + 2
        (10, Affinity::After, 11), // after all: 10 - 1 + 2
    ];
    for &(offset, affinity, want) in cases {
        assert_eq!(
            transform_anchor_through_edits(at(offset, affinity), &edits).offset,
            want,
            "offset {offset} {affinity:?}"
        );
    }
}

#[test]
fn committed_transaction_transform_matches_pure_function() {
    let mut doc = Document::from_string("abcdefghij");
    let txn = doc
        .commit(
            crate::kernel::text_engine::transaction::TransactionBuilder::new()
                .replace(2..4, "Z")
                .insert(8, "QQ"),
        )
        .unwrap()
        .unwrap();
    assert_eq!(doc.text(), "abZefghQQij");
    let edits: Vec<Edit> = txn.edits.iter().map(|ce| ce.edit.clone()).collect();
    for offset in 0usize..=10 {
        for affinity in [Affinity::Before, Affinity::After] {
            let a = at(offset, affinity);
            assert_eq!(
                txn.transform_anchor(a),
                transform_anchor_through_edits(a, &edits),
                "offset {offset} {affinity:?}"
            );
        }
    }
}

#[test]
fn undo_returns_typed_cursor_to_insertion_start() {
    // Cursor sits After the insertion point; typing "abc" moves it to 3.
    // Undo (inverse transaction) must bring it exactly back to 0.
    let mut doc = Document::new();
    let txn = doc
        .commit(
            crate::kernel::text_engine::transaction::TransactionBuilder::new()
                .insert(0, "abc"),
        )
        .unwrap()
        .unwrap();
    let cursor = txn.transform_anchor(at(0, Affinity::After));
    assert_eq!(cursor.offset, 3);

    let undo = doc.commit(txn.invert(TransactionOrigin::Undo)).unwrap().unwrap();
    assert_eq!(doc.text(), "");
    assert_eq!(undo.transform_anchor(cursor).offset, 0);
}
