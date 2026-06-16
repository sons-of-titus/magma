use magma::kernel::render::surface::{Surface, Style};

fn main() {
    // Benchmark: Surface::new at 80x24
    {
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = Surface::new(80, 24);
        }
        let elapsed = start.elapsed();
        println!(
            "Surface::new 80x24 x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: Surface::new at 200x60
    {
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            let _ = Surface::new(200, 60);
        }
        let elapsed = start.elapsed();
        println!(
            "Surface::new 200x60 x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: clear on 80x24 surface
    {
        let mut s = Surface::new(80, 24);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            s.clear();
        }
        let elapsed = start.elapsed();
        println!(
            "clear 80x24 x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: clear on 200x60 surface
    {
        let mut s = Surface::new(200, 60);
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            s.clear();
        }
        let elapsed = start.elapsed();
        println!(
            "clear 200x60 x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: set_cell at sequential positions
    {
        let mut s = Surface::new(80, 24);
        let style = Style { fg: (255, 0, 0), bg: (0, 0, 0), bold: false, italic: false, underline: false, strikethrough: false, dim: false };
        let start = std::time::Instant::now();
        for y in 0..24 {
            for x in 0..80 {
                s.set_cell(x, y, 'A', Some(style));
            }
        }
        let elapsed = start.elapsed();
        println!(
            "set_cell full grid 80x24: {:?} ({:.3} µs/cell)",
            elapsed,
            elapsed.as_micros() as f64 / (80.0 * 24.0)
        );
    }

    // Benchmark: set_cell with None style (no-op style branch)
    {
        let mut s = Surface::new(80, 24);
        let start = std::time::Instant::now();
        for y in 0..24 {
            for x in 0..80 {
                s.set_cell(x, y, 'B', None);
            }
        }
        let elapsed = start.elapsed();
        println!(
            "set_cell full grid no-style 80x24: {:?} ({:.3} µs/cell)",
            elapsed,
            elapsed.as_micros() as f64 / (80.0 * 24.0)
        );
    }

    // Benchmark: set_cell out of bounds (x too large)
    {
        let mut s = Surface::new(80, 24);
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            s.set_cell(999, 999, 'X', None);
        }
        let elapsed = start.elapsed();
        println!(
            "set_cell out-of-bounds x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: set_text at sequential positions
    {
        let mut s = Surface::new(80, 24);
        let start = std::time::Instant::now();
        for y in 0..24 {
            s.set_text(0, y, "hello world this is a line of text that fits", None);
        }
        let elapsed = start.elapsed();
        println!(
            "set_text 24 rows x 50 chars: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 24.0
        );
    }

    // Benchmark: rows allocation on 80x24
    {
        let s = Surface::new(80, 24);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = s.rows();
        }
        let elapsed = start.elapsed();
        println!(
            "rows() 80x24 x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: rows allocation on 200x60
    {
        let s = Surface::new(200, 60);
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            let _ = s.rows();
        }
        let elapsed = start.elapsed();
        println!(
            "rows() 200x60 x1000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: resize surface
    {
        let mut s = Surface::new(80, 24);
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            s.resize(120, 40);
            s.resize(80, 24);
        }
        let elapsed = start.elapsed();
        println!(
            "resize 80x24→120x40→80x24 x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: cell lookup at sequential positions
    {
        let s = Surface::new(80, 24);
        let start = std::time::Instant::now();
        for y in 0..24 {
            for x in 0..80 {
                let _ = s.cell(x, y);
            }
        }
        let elapsed = start.elapsed();
        println!(
            "cell() full grid 80x24: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / (80.0 * 24.0)
        );
    }
}
