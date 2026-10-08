//! Phase 1.5 property tests (docs/core-redesign.md): random edit sequences
//! must keep the kernel Document identical to a plain-`String` reference
//! after every operation, with anchors, line index, revisions and inversion
//! verified against independently computed oracles.
//!
//! Deterministic: every failure names its seed.

use crate::kernel::text_engine::anchor::{Affinity, Anchor};
use crate::kernel::text_engine::document::Document;
use crate::kernel::text_engine::edit::Edit;
use crate::kernel::text_engine::revision::Revision;
use crate::kernel::text_engine::transaction::{TransactionBuilder, TransactionOrigin};

/// xorshift64* — deterministic, seedable, no external crates.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// Unicode stress palette: ASCII, newlines, 2-byte, 3-byte, 4-byte code
/// points, a combining sequence, and multi-char chunks.
const CHUNKS: &[&str] = &[
    "a", "b", " ", "\n", "hello", "world\n", "é", "中", "😀", "é\u{301}", "x\ny", "ab\n",
    "\n\n", "ß", "𝕄",
];

fn boundaries(text: &str) -> Vec<usize> {
    let mut b: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    b.push(text.len());
    b.sort_unstable();
    b.dedup();
    b
}

/// A random non-overlapping edit set in pre-transaction coordinates,
/// sorted ascending (builder-legal input).
fn random_edits(rng: &mut Rng, text: &str) -> Vec<Edit> {
    let b = boundaries(text);
    let mut edits: Vec<Edit> = Vec::new();
    for _ in 0..1 + rng.below(4) {
        let i = b[rng.below(b.len())];
        let j = b[rng.below(b.len())];
        let (s, e) = if i <= j { (i, j) } else { (j, i) };
        let chunk = CHUNKS[rng.below(CHUNKS.len())];
        edits.push(match (rng.below(3), s < e) {
            (0, false) => Edit::insert(s, chunk),
            (1, true) => Edit::delete(s..e),
            (_, true) => Edit::replace(s..e, chunk),
            (_, false) => Edit::insert(s, chunk),
        });
    }
    edits.sort_by_key(|ed| (ed.start(), ed.end()));
    let mut out: Vec<Edit> = Vec::new();
    let mut last_end = 0usize;
    for ed in edits {
        if ed.start() >= last_end {
            last_end = ed.end();
            out.push(ed);
        }
    }
    out
}

/// Reference oracle: apply ascending, disjoint edits back-to-front.
fn apply_reference(text: &mut String, edits: &[Edit]) {
    for ed in edits.iter().rev() {
        text.replace_range(ed.start()..ed.end(), &ed.inserted_text);
    }
}

/// Exact expected position for an anchor that lies strictly outside every
/// edit's closed range: shift by the deltas of all edits entirely before it.
fn unaffected_shift(edits: &[Edit], offset: usize) -> Option<i64> {
    if edits
        .iter()
        .any(|e| offset >= e.start() && offset <= e.end())
    {
        return None;
    }
    Some(
        edits
            .iter()
            .filter(|e| e.end() < offset)
            .map(|e| e.inserted_len() as i64 - (e.end() - e.start()) as i64)
            .sum(),
    )
}

fn sample_anchors(rng: &mut Rng, text: &str, count: usize) -> Vec<Anchor> {
    let b = boundaries(text);
    let mut idxs: Vec<usize> = Vec::new();
    for _ in 0..count {
        idxs.push(rng.below(b.len()));
    }
    idxs.sort_unstable();
    idxs.dedup();
    idxs
        .into_iter()
        .map(|i| Anchor {
            offset: b[i],
            affinity: if rng.flip() { Affinity::Before } else { Affinity::After },
        })
        .collect()
}

impl Rng {
    fn flip(&mut self) -> bool {
        self.next() & 1 == 0
    }
}

fn random_reference(rng: &mut Rng) -> String {
    (0..rng.below(40))
        .map(|_| CHUNKS[rng.below(CHUNKS.len())])
        .collect()
}

fn check_line_index(doc: &Document, reference: &str) {
    let li = doc.line_index();
    let expected_count = if reference.is_empty() {
        1
    } else {
        reference.lines().count()
            + usize::from(reference.ends_with('\n'))
    };
    assert_eq!(li.line_count(), expected_count, "line count for {reference:?}");

    let newline_offsets: Vec<usize> = reference
        .bytes()
        .enumerate()
        .filter(|(_, b)| *b == b'\n')
        .map(|(i, _)| i)
        .collect();

    for offset in boundaries(reference) {
        let expected = newline_offsets.partition_point(|&p| p < offset);
        assert_eq!(li.line_of(offset), expected, "line_of({offset}) in {reference:?}");
    }

    for (line, content) in reference.lines().enumerate() {
        let span = li.line_span(line).expect("line span in range");
        assert_eq!(&reference[span], content, "line {line} span in {reference:?}");
    }
    // Line k starts right after the (k-1)-th '\n'; a trailing '\n' starts
    // one final empty line.
    for line in 0..expected_count {
        let want = if line == 0 {
            0
        } else {
            newline_offsets.get(line - 1).map(|&p| p + 1).unwrap_or(reference.len())
        };
        assert_eq!(li.line_start(line), Some(want), "line_start({line}) in {reference:?}");
    }
}

fn check_txn_structure(txn_edits: &[crate::kernel::text_engine::transaction::CommittedEdit], pre_ref: &str) {
    for pair in txn_edits.windows(2) {
        assert!(
            pair[0].edit.end() < pair[1].edit.start(),
            "committed edits must be ascending and strictly non-abutting"
        );
    }
    for ce in txn_edits {
        assert_eq!(ce.old_text, &pre_ref[ce.edit.start()..ce.edit.end()], "old_text capture");
    }
}

#[test]
fn random_edit_sequences_match_string_reference() {
    for seed in 1..=100u64 {
        let mut rng = Rng::new(seed);
        let mut reference = random_reference(&mut rng);
        let mut doc = Document::from_string(&reference);
        let mut expected_rev = 0u64;

        for it in 0..50 {
            let edits = random_edits(&mut rng, &reference);
            if edits.is_empty() {
                continue;
            }
            let pre_ref = reference.clone();
            let anchors = sample_anchors(&mut rng, &pre_ref, 6);

            let builder = edits
                .iter()
                .fold(TransactionBuilder::new(), |b, e| b.push(e.clone()));
            let txn = doc
                .commit(builder)
                .unwrap_or_else(|e| panic!("seed {seed} iter {it}: {e}"))
                .expect("generated edits are never all no-ops");

            apply_reference(&mut reference, &edits);
            expected_rev += 1;

            let ctx = format!("seed {seed} iter {it}");
            assert_eq!(doc.text(), reference, "text ({ctx})");
            assert_eq!(doc.revision(), Revision(expected_rev), "revision ({ctx})");
            assert_eq!(txn.revision_before.0, expected_rev - 1, "txn before ({ctx})");
            assert_eq!(txn.revision_after.0, expected_rev, "txn after ({ctx})");
            assert_eq!(doc.history().len() as u64, expected_rev, "history ({ctx})");
            check_txn_structure(&txn.edits, &pre_ref);

            let mut moved_before: Vec<usize> = Vec::new();
            let mut moved_after: Vec<usize> = Vec::new();
            for a in &anchors {
                let m = txn.transform_anchor(*a);
                if let Some(shift) = unaffected_shift(&edits, a.offset) {
                    let want = (a.offset as i64 + shift) as usize;
                    assert_eq!(m.offset, want, "unaffected anchor {} {:?} ({ctx})", a.offset, a.affinity);
                }
                assert!(m.offset <= reference.len(), "anchor in bounds ({ctx})");
                assert!(reference.is_char_boundary(m.offset), "anchor on char boundary ({ctx})");
                match a.affinity {
                    Affinity::Before => moved_before.push(m.offset),
                    Affinity::After => moved_after.push(m.offset),
                }
            }
            // Ordering is preserved within an affinity class; mixed-affinity
            // anchors may legitimately swap when the span between them dies.
            assert!(
                moved_before.windows(2).all(|w| w[0] <= w[1]),
                "Before-anchor ordering preserved ({ctx})"
            );
            assert!(
                moved_after.windows(2).all(|w| w[0] <= w[1]),
                "After-anchor ordering preserved ({ctx})"
            );

            check_line_index(&doc, &reference);

            let chain = doc.history();
            assert!(
                chain.windows(2).all(|w| w[0].revision_after == w[1].revision_before),
                "revision chain continuous ({ctx})"
            );
        }
    }
}

#[test]
fn undo_round_trips_restore_text_and_anchors() {
    for seed in 1..=50u64 {
        let mut rng = Rng::new(seed);
        let mut reference = random_reference(&mut rng);
        let mut doc = Document::from_string(&reference);

        for it in 0..40 {
            let edits = random_edits(&mut rng, &reference);
            if edits.is_empty() {
                continue;
            }
            let anchors = sample_anchors(&mut rng, &reference, 5);
            let pre_forward = reference.clone();
            let builder = edits
                .iter()
                .fold(TransactionBuilder::new(), |b, e| b.push(e.clone()));
            let txn = doc.commit(builder).unwrap().unwrap();
            apply_reference(&mut reference, &edits);

            if it % 4 == 3 {
                let inverse = txn.inverse_edits();
                let undo = doc
                    .commit(txn.invert(TransactionOrigin::Undo))
                    .unwrap()
                    .unwrap();
                apply_reference(&mut reference, &inverse);

                let ctx = format!("seed {seed} iter {it}");
                assert_eq!(doc.text(), reference, "undo text ({ctx})");
                assert_eq!(doc.text(), pre_forward, "undo restores pre-commit text ({ctx})");
                assert_eq!(undo.origin, TransactionOrigin::Undo);
                assert_eq!(undo.revision_before, txn.revision_after, "undo continues the chain ({ctx})");

                for a in &anchors {
                    let after_forward = txn.transform_anchor(*a);
                    let after_undo = undo.transform_anchor(after_forward);
                    if let Some(shift) = unaffected_shift(&inverse, after_forward.offset) {
                        let want = (after_forward.offset as i64 + shift) as usize;
                        assert_eq!(
                            after_undo.offset, want,
                            "anchor {} {:?} through undo ({ctx})",
                            a.offset, a.affinity
                        );
                    }
                    assert!(after_undo.offset <= reference.len());
                    assert!(reference.is_char_boundary(after_undo.offset));
                }
            }
        }
    }
}

#[test]
fn parallel_transaction_equals_one_commit_per_edit() {
    // Parallel edits in pre-transaction coordinates are equivalent to
    // committing each edit individually, back-to-front.
    for seed in 1..=60u64 {
        let mut rng = Rng::new(seed);
        let reference = random_reference(&mut rng);
        let edits = random_edits(&mut rng, &reference);
        if edits.is_empty() {
            continue;
        }

        let mut parallel = Document::from_string(&reference);
        let mut sequential = Document::from_string(&reference);

        let builder = edits
            .iter()
            .fold(TransactionBuilder::new(), |b, e| b.push(e.clone()));
        parallel.commit(builder).unwrap().unwrap();
        for ed in edits.iter().rev() {
            sequential
                .commit(TransactionBuilder::new().push(ed.clone()))
                .unwrap()
                .unwrap();
        }

        assert_eq!(
            parallel.text(),
            sequential.text(),
            "seed {seed}: parallel and sequential diverged"
        );
        assert_eq!(parallel.revision(), Revision(1));
        assert_eq!(sequential.revision(), Revision(edits.len() as u64));
    }
}

#[test]
fn large_documents_and_huge_lines() {
    // One huge line + a second line, then edits at BOF, inside, EOF, and
    // across the newline boundary.
    let huge_line = "x".repeat(100_000);
    let start = format!("{huge_line}\nend\n");
    let mut reference = start.clone();
    let mut doc = Document::from_string(&reference);

    let mut edits = [
        Edit::insert(0, "BOF"),
        Edit::insert(reference.len(), "!"),              // at EOF
        Edit::replace(50_000..50_010, "middle"),          // inside the huge line
        Edit::replace(huge_line.len() - 2..huge_line.len() + 3, "SEAM"), // across the '\n'
    ];
    edits.sort_by_key(|e| (e.start(), e.end()));
    let builder = edits
        .iter()
        .fold(TransactionBuilder::new(), |b, e| b.push(e.clone()));
    doc.commit(builder).unwrap().unwrap();
    apply_reference(&mut reference, &edits);

    assert_eq!(doc.len(), reference.len());
    assert_eq!(doc.text(), reference);
    assert_eq!(doc.line_index().line_of(1), 0);
    assert_eq!(doc.line_index().line_of(doc.len()), 1);
    check_line_index(&doc, &reference);
}

#[test]
fn empty_to_content_and_back() {
    let mut doc = Document::new();
    assert_eq!(doc.text(), "");
    let txn = doc
        .commit(TransactionBuilder::new().insert(0, "héllo\nwörld"))
        .unwrap()
        .unwrap();
    assert_eq!(doc.revision(), Revision(1));
    check_line_index(&doc, "héllo\nwörld");

    doc.commit(txn.invert(TransactionOrigin::Undo)).unwrap().unwrap();
    assert_eq!(doc.text(), "");
    assert_eq!(doc.revision(), Revision(2), "undo bumps; revisions never reused");
    check_line_index(&doc, "");
}
