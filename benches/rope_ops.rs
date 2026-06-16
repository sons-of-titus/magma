use magma::kernel::text_engine::rope::Rope;

fn make_rope(lines: usize) -> Rope {
    let content = "hello world this is a line of text\n".repeat(lines);
    Rope::from_string(&content)
}

fn main() {
    // Benchmark: insert at various positions in 10k-line rope
    {
        let mut r = make_rope(10_000);
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let pos = (i * 10) % r.len().max(1);
            r.insert(pos, "x");
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line rope insert x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: delete at various positions in 10k-line rope
    {
        let mut r = make_rope(10_000);
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let pos = (i * 10) % r.len().max(1);
            if pos + 1 <= r.len() {
                r.delete(pos, pos + 1);
            }
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line rope delete x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: slice small region in 100k-line rope
    {
        let r = make_rope(100_000);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = r.slice(100, 200);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line rope small slice x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: full slice in 100k-line rope
    {
        let r = make_rope(100_000);
        let len = r.len();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = r.slice(0, len);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line rope full slice x100: {:?} ({:.1} ms/op)",
            elapsed,
            elapsed.as_millis() as f64 / 100.0
        );
    }

    // Benchmark: char_at random access in 100k-line rope
    {
        let r = make_rope(100_000);
        let len = r.len();
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            let pos = (i * 4099) % len;
            let _ = r.char_at(pos);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line rope random char_at x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: line_count in 100k-line rope
    {
        let r = make_rope(100_000);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = r.line_count();
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line rope line_count x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: random line fetch in 100k-line rope
    {
        let r = make_rope(100_000);
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let n = (i * 97) % 100_000;
            let _ = r.line(n);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line rope random line x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: line_start_offset in 100k-line rope
    {
        let r = make_rope(100_000);
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            let n = (i * 37) % 100_000;
            let _ = r.line_start_offset(n);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line rope line_start_offset x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: all_lines in 10k-line rope
    {
        let r = make_rope(10_000);
        let start = std::time::Instant::now();
        let _ = r.all_lines();
        let elapsed = start.elapsed();
        println!(
            "10k-line rope all_lines: {:?} ({:.3} ms)",
            elapsed,
            elapsed.as_millis() as f64
        );
    }

    // Benchmark: prepend (insert at 0) in 10k-line rope
    {
        let mut r = make_rope(10_000);
        let start = std::time::Instant::now();
        for _ in 0..100 {
            r.insert(0, "PREFIX\n");
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line rope prepend x100: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100.0
        );
    }

    // Benchmark: append (insert at end) in 10k-line rope
    {
        let mut r = make_rope(10_000);
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            r.insert(r.len(), "APPEND\n");
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line rope append x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }
}
