use magma::kernel::state::id::BufferId;
use magma::kernel::text_engine::Buffer;

fn make_buffer(lines: usize) -> Buffer {
    let content = "hello world this is a line of text\n".repeat(lines);
    Buffer::from_string(BufferId(0), "bench", &content)
}

fn main() {
    // Benchmark: sequential inserts at 10k lines
    {
        let mut buf = make_buffer(10_000);
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let pos = (i * 10) % buf.len().max(1);
            buf.insert(pos, "x");
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line insert x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: sequential inserts at 100k lines
    {
        let mut buf = make_buffer(100_000);
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let pos = (i * 100) % buf.len().max(1);
            buf.insert(pos, "x");
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line insert x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: sequential deletes at 10k lines
    {
        let mut buf = make_buffer(10_000);
        let len = buf.len();
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let pos = (i * 10) % (buf.len().saturating_sub(1)).max(1);
            if pos + 1 <= buf.len() {
                buf.delete(pos, pos + 1);
            }
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line delete x1000 (started at {} bytes): {:?} ({:.1} µs/op)",
            len,
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: slice (read) at 100k lines
    {
        let buf = make_buffer(100_000);
        let len = buf.len();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = buf.slice(0, len);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line full slice x100: {:?} ({:.1} ms/op)",
            elapsed,
            elapsed.as_millis() as f64 / 100.0
        );
    }

    // Benchmark: line_count at 100k lines
    {
        let buf = make_buffer(100_000);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = buf.line_count();
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line line_count x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: line fetch at 100k lines (random access)
    {
        let buf = make_buffer(100_000);
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let n = (i * 97) % 100_000;
            let _ = buf.line(n);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line random line fetch x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: lines (all) at 10k lines
    {
        let buf = make_buffer(10_000);
        let start = std::time::Instant::now();
        let _ = buf.lines();
        let elapsed = start.elapsed();
        println!(
            "10k-line all lines fetch: {:?} ({:.3} ms)",
            elapsed,
            elapsed.as_millis() as f64
        );
    }

    // Benchmark: char_at at 100k lines (random access)
    {
        let buf = make_buffer(100_000);
        let len = buf.len();
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            let pos = (i * 4099) % len;
            let _ = buf.char_at(pos);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line random char_at x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: replace in middle at 10k lines
    {
        let mut buf = make_buffer(10_000);
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            let pos = (i * 100) % buf.len().max(1);
            buf.replace(pos, pos + 1, "Y");
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line single-char replace x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: undo/redo chain at 10k lines
    {
        let mut buf = make_buffer(10_000);
        for i in 0..100 {
            let pos = (i * 50) % buf.len().max(1);
            buf.insert(pos, "X");
        }
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let _ = buf.undo();
        }
        for _ in 0..100 {
            let _ = buf.redo();
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line undo+redo x100 each: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 200.0
        );
    }

    // Benchmark: prepend (insert at 0) at 10k lines
    {
        let mut buf = make_buffer(10_000);
        let start = std::time::Instant::now();
        for _ in 0..100 {
            buf.insert(0, "PREFIX\n");
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line prepend x100: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100.0
        );
    }

    // Benchmark: append (insert at end) at 10k lines
    {
        let mut buf = make_buffer(10_000);
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            buf.insert(buf.len(), "APPEND\n");
        }
        let elapsed = start.elapsed();
        println!(
            "10k-line append x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }
}
