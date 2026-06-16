use magma::kernel::text_engine::undo::UndoTree;
use magma::kernel::state::id::MarkId;

fn make_marks(n: usize) -> Vec<(MarkId, usize)> {
    (0..n).map(|i| (MarkId(i as u64), i * 100)).collect()
}

fn main() {
    // Benchmark: record single ops
    {
        let mut tree = UndoTree::new(1000);
        let marks = make_marks(10);
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            tree.record(
                magma::kernel::text_engine::undo::UndoOp::Insert {
                    offset: i,
                    text: "x".into(),
                },
                marks.clone(),
            );
        }
        let elapsed = start.elapsed();
        println!(
            "record 10000 single ops: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: record ops that merge (within 200ms window)
    {
        let mut tree = UndoTree::new(1000);
        let marks = make_marks(10);
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            tree.record(
                magma::kernel::text_engine::undo::UndoOp::Insert {
                    offset: i,
                    text: "x".into(),
                },
                marks.clone(),
            );
        }
        let elapsed = start.elapsed();
        println!(
            "record 10000 merging ops: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: undo + redo cycle
    {
        let mut tree = UndoTree::new(1000);
        let marks = make_marks(10);
        for i in 0..500 {
            tree.record(
                magma::kernel::text_engine::undo::UndoOp::Insert {
                    offset: i,
                    text: "hello".into(),
                },
                marks.clone(),
            );
        }
        let start = std::time::Instant::now();
        for _ in 0..500 {
            let _ = tree.undo();
        }
        for _ in 0..500 {
            let _ = tree.redo();
        }
        let elapsed = start.elapsed();
        println!(
            "undo+redo 500 ops each: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: open_session + close_session
    {
        let mut tree = UndoTree::new(1000);
        let marks = make_marks(10);
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            tree.open_session(
                magma::kernel::text_engine::undo::UndoOp::Insert {
                    offset: 0,
                    text: "x".into(),
                },
                marks.clone(),
            );
            tree.close_session();
        }
        let elapsed = start.elapsed();
        println!(
            "open+close session x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: record with large mark sets
    {
        let mut tree = UndoTree::new(1000);
        let big_marks = make_marks(1000);
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            tree.record(
                magma::kernel::text_engine::undo::UndoOp::Delete {
                    offset: 0,
                    text: "deleted text here".into(),
                },
                big_marks.clone(),
            );
        }
        let elapsed = start.elapsed();
        println!(
            "record 1000 ops with 1000 marks each: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: can_undo / can_redo
    {
        let mut tree = UndoTree::new(100);
        let marks = make_marks(5);
        for i in 0..50 {
            tree.record(
                magma::kernel::text_engine::undo::UndoOp::Insert {
                    offset: i, text: "x".into(),
                },
                marks.clone(),
            );
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

    // Benchmark: mark_saved + is_modified_since_save
    {
        let mut tree = UndoTree::new(100);
        let marks = make_marks(5);
        for i in 0..10 {
            tree.record(
                magma::kernel::text_engine::undo::UndoOp::Insert {
                    offset: i, text: "x".into(),
                },
                marks.clone(),
            );
        }
        tree.mark_saved();
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = tree.is_modified_since_save();
        }
        let elapsed = start.elapsed();
        println!(
            "is_modified_since_save x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: clear
    {
        let mut tree = UndoTree::new(100);
        let marks = make_marks(5);
        for i in 0..100 {
            tree.record(
                magma::kernel::text_engine::undo::UndoOp::Insert {
                    offset: i, text: "x".into(),
                },
                marks.clone(),
            );
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

    // Benchmark: max_groups eviction
    {
        let marks = make_marks(5);
        let start = std::time::Instant::now();
        {
            let mut tree = UndoTree::new(50);
            for i in 0..10_000 {
                tree.record(
                    magma::kernel::text_engine::undo::UndoOp::Insert {
                        offset: i, text: "x".into(),
                    },
                    marks.clone(),
                );
            }
        }
        let elapsed = start.elapsed();
        println!(
            "record 10000 ops with max 50 groups (eviction): {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }
}
