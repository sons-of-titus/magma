use magma::kernel::text_engine::edit::Edit;
use magma::kernel::text_engine::revision::Revision;
use magma::kernel::text_engine::transaction::{
    CommittedEdit, EditTransaction, TransactionOrigin,
};
use magma::kernel::text_engine::undo::UndoHistory;

fn insert_txn(rev: u64, offset: usize, text: &str) -> EditTransaction {
    EditTransaction {
        revision_before: Revision(rev),
        revision_after: Revision(rev + 1),
        edits: vec![CommittedEdit {
            edit: Edit::insert(offset, text),
            old_text: String::new(),
        }],
        origin: TransactionOrigin::User,
    }
}

fn main() {
    // Benchmark: record single ops (each its own undo step)
    {
        let mut tree = UndoHistory::new();
        let start = std::time::Instant::now();
        for i in 0..10_000u64 {
            tree.record(insert_txn(i, i as usize, "x"), i as usize, i as usize + 1);
        }
        let elapsed = start.elapsed();
        println!(
            "record 10000 single ops: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: record ops inside one explicit group (one undo step)
    {
        let mut tree = UndoHistory::new();
        tree.open_group();
        let start = std::time::Instant::now();
        for i in 0..10_000u64 {
            tree.record(insert_txn(i, i as usize, "x"), i as usize, i as usize + 1);
        }
        let elapsed = start.elapsed();
        println!(
            "record 10000 grouped ops: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: undo + redo cycle
    {
        let mut tree = UndoHistory::new();
        for i in 0..500u64 {
            tree.record(insert_txn(i, i as usize, "hello"), i as usize, i as usize + 5);
        }
        let start = std::time::Instant::now();
        for _ in 0..500 {
            if let Some((start_idx, _, _)) = tree.undo_step() {
                tree.finish_undo(start_idx);
            }
        }
        for _ in 0..500 {
            if let Some((_, end_idx, _)) = tree.redo_step() {
                tree.finish_redo(end_idx);
            }
        }
        let elapsed = start.elapsed();
        println!(
            "undo+redo 500 ops each: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: open_group + close_group
    {
        let mut tree = UndoHistory::new();
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            tree.open_group();
            tree.close_group();
        }
        let elapsed = start.elapsed();
        println!(
            "open+close group x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: record with large edit sets
    {
        let mut tree = UndoHistory::new();
        let big: Vec<CommittedEdit> = (0..1_000)
            .map(|i| CommittedEdit {
                edit: Edit::delete(i * 20..i * 20 + 18),
                old_text: "deleted text here".to_string(),
            })
            .collect();
        let start = std::time::Instant::now();
        for i in 0..1_000u64 {
            tree.record(
                EditTransaction {
                    revision_before: Revision(i),
                    revision_after: Revision(i + 1),
                    edits: big.clone(),
                    origin: TransactionOrigin::User,
                },
                0,
                0,
            );
        }
        let elapsed = start.elapsed();
        println!(
            "record 1000 ops with 1000 edits each: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: can_undo / can_redo
    {
        let mut tree = UndoHistory::new();
        for i in 0..50u64 {
            tree.record(insert_txn(i, i as usize, "x"), i as usize, i as usize + 1);
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = tree.can_undo();
            let _ = tree.can_redo();
        }
        let elapsed = start.elapsed();
        println!(
            "can_undo+can_redo x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: mark_saved_at_cursor + is_modified
    {
        let mut tree = UndoHistory::new();
        for i in 0..10u64 {
            tree.record(insert_txn(i, i as usize, "x"), i as usize, i as usize + 1);
        }
        tree.mark_saved_at_cursor(tree.cursor_pos());
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = tree.is_modified();
        }
        let elapsed = start.elapsed();
        println!(
            "is_modified x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: clear
    {
        let mut tree = UndoHistory::new();
        for i in 0..100u64 {
            tree.record(insert_txn(i, i as usize, "x"), i as usize, i as usize + 1);
        }
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            tree.clear();
        }
        let elapsed = start.elapsed();
        println!(
            "clear x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: entry eviction at the 10000-entry cap
    {
        let start = std::time::Instant::now();
        {
            let mut tree = UndoHistory::new();
            for i in 0..20_000u64 {
                tree.record(insert_txn(i, i as usize, "x"), i as usize, i as usize + 1);
            }
        }
        let elapsed = start.elapsed();
        println!(
            "record 20000 ops (cap 10000, eviction): {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 20_000.0
        );
    }
}
