use magma::kernel::keymap::KeymapManager;

fn main() {
    // Benchmark: resolve in empty keymap (miss)
    {
        let km = KeymapManager::new();
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = km.resolve("x");
        }
        let elapsed = start.elapsed();
        println!(
            "resolve (empty, miss) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: resolve from global layer (hit)
    {
        let mut km = KeymapManager::new();
        for i in 0..200 {
            km.set(&format!("key-{}", i), &format!("cmd-{}", i));
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = km.resolve("key-100");
        }
        let elapsed = start.elapsed();
        println!(
            "resolve (200 global, hit) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: resolve from global layer (miss)
    {
        let mut km = KeymapManager::new();
        for i in 0..200 {
            km.set(&format!("key-{}", i), &format!("cmd-{}", i));
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = km.resolve("nonexistent");
        }
        let elapsed = start.elapsed();
        println!(
            "resolve (200 global, miss) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: resolve with active layers (hit in highest-priority layer)
    {
        let mut km = KeymapManager::new();
        km.set("key", "global-cmd");
        km.set_layer("insert", "key", "insert-cmd");
        km.set_layer("visual", "key", "visual-cmd");
        km.push_layer("insert");
        km.push_layer("visual");
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = km.resolve("key");
        }
        let elapsed = start.elapsed();
        println!(
            "resolve (2 active layers, hit top) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: resolve with buffer-local bindings
    {
        let mut km = KeymapManager::new();
        km.set_layer("insert", "j", "insert-cmd");
        km.push_layer("insert");
        km.set_buffer_local(42, "j", "local-cmd");
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = km.resolve_for_buffer("j", Some(42));
        }
        let elapsed = start.elapsed();
        println!(
            "resolve_for_buffer (buffer-local hit) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: resolve with many layers (10 active, all miss → global hit)
    {
        let mut km = KeymapManager::new();
        km.set("fallback", "global-cmd");
        for i in 0..10 {
            km.set_layer(&format!("layer-{}", i), &format!("lk-{}", i), &format!("lcmd-{}", i));
            km.push_layer(&format!("layer-{}", i));
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = km.resolve("fallback");
        }
        let elapsed = start.elapsed();
        println!(
            "resolve (10 layers miss → global hit) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: push_layer + pop_layer cycle
    {
        let mut km = KeymapManager::new();
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            km.push_layer(&format!("layer-{}", i % 20));
            km.pop_layer(&format!("layer-{}", i % 20));
        }
        let elapsed = start.elapsed();
        println!(
            "push+pop layer cycle x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: set global bindings
    {
        let mut km = KeymapManager::new();
        let start = std::time::Instant::now();
        for i in 0..10_000 {
            km.set(&format!("key-{}", i), &format!("cmd-{}", i));
        }
        let elapsed = start.elapsed();
        println!(
            "set 10000 global bindings: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }
}
