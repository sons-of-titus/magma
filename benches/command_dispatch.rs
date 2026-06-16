use magma::kernel::command::{CommandRegistry, args::{ArgSpec, ArgType, ArgValue}};

fn main() {
    // Benchmark: register N commands
    {
        let mut reg = CommandRegistry::new();
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            reg.register_fn(
                &format!("cmd-{}", i),
                "bench command",
                vec![],
                |_, _| Ok(()),
            );
        }
        let elapsed = start.elapsed();
        println!(
            "register 1000 commands: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: exists lookup (hit)
    {
        let mut reg = CommandRegistry::new();
        for i in 0..500 {
            reg.register_fn(
                &format!("cmd-{}", i),
                "bench command",
                vec![],
                |_, _| Ok(()),
            );
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = reg.exists("cmd-250");
        }
        let elapsed = start.elapsed();
        println!(
            "exists (hit) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: exists lookup (miss)
    {
        let mut reg = CommandRegistry::new();
        for i in 0..500 {
            reg.register_fn(
                &format!("cmd-{}", i),
                "bench command",
                vec![],
                |_, _| Ok(()),
            );
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = reg.exists("nonexistent");
        }
        let elapsed = start.elapsed();
        println!(
            "exists (miss) x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: list all commands (with 500 registered)
    {
        let mut reg = CommandRegistry::new();
        for i in 0..500 {
            reg.register_fn(
                &format!("cmd-{}", i),
                "bench command",
                vec![],
                |_, _| Ok(()),
            );
        }
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = reg.list();
        }
        let elapsed = start.elapsed();
        println!(
            "list 500 commands x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: register with args
    {
        let mut reg = CommandRegistry::new();
        let start = std::time::Instant::now();
        for i in 0..1_000 {
            reg.register_fn(
                &format!("cmd-{}", i),
                "bench command with args",
                vec![
                    ArgSpec::new("arg1", ArgType::String),
                    ArgSpec::optional("arg2", ArgType::String, ArgValue::String("default".into())),
                ],
                |_, _| Ok(()),
            );
        }
        let elapsed = start.elapsed();
        println!(
            "register 1000 commands with args: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: len/is_empty on empty registry
    {
        let reg = CommandRegistry::new();
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = reg.len();
            let _ = reg.is_empty();
        }
        let elapsed = start.elapsed();
        println!(
            "len+is_empty on empty reg x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }

    // Benchmark: len on populated registry
    {
        let mut reg = CommandRegistry::new();
        for i in 0..500 {
            reg.register_fn(
                &format!("cmd-{}", i),
                "bench command",
                vec![],
                |_, _| Ok(()),
            );
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = reg.len();
        }
        let elapsed = start.elapsed();
        println!(
            "len on 500-command reg x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }
}
