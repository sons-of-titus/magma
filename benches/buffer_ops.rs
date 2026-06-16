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

    // Benchmark: cursor movement at 100k lines
    {
        let mut buf = make_buffer(100_000);
        let len = buf.len();
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            buf.set_cursor((i * 37) % len);
        }
        let elapsed = start.elapsed();
        println!(
            "100k-line cursor-set x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }
}
