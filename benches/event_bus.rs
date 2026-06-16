use std::collections::HashMap;
use magma::kernel::event::EventBus;

fn main() {
    // Benchmark: emit + drain_and_dispatch with N subscribers
    {
        let mut bus = EventBus::new();
        for i in 0..100 {
            bus.on("test-event", move |_| {
                let _ = i;
                None
            });
        }
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            bus.emit("test-event", HashMap::new());
            bus.drain_and_dispatch();
        }
        let elapsed = start.elapsed();
        println!(
            "100 subs emit+dispatch x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: emit + drain_and_dispatch with 1 subscriber
    {
        let mut bus = EventBus::new();
        bus.on("simple-event", |_| None);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            bus.emit("simple-event", HashMap::new());
            bus.drain_and_dispatch();
        }
        let elapsed = start.elapsed();
        println!(
            "1 sub emit+dispatch x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: emit + drain_and_dispatch with Before/After hooks
    {
        let mut bus = EventBus::new();
        bus.on("hooked-event", |_| None);
        bus.add_hook("hooked-event", magma::kernel::event::HookPhase::Before, |_| None);
        bus.add_hook("hooked-event", magma::kernel::event::HookPhase::After, |_| None);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            bus.emit("hooked-event", HashMap::new());
            bus.drain_and_dispatch();
        }
        let elapsed = start.elapsed();
        println!(
            "1 sub + 2 hooks emit+dispatch x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: multiple event types with sparse subscribers
    {
        let mut bus = EventBus::new();
        for i in 0..20 {
            bus.on(&format!("event-{}", i), move |_| {
                let _ = i;
                None
            });
        }
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            for i in 0..20 {
                bus.emit(&format!("event-{}", i), HashMap::new());
            }
            bus.drain_and_dispatch();
        }
        let elapsed = start.elapsed();
        println!(
            "20 event types x 1 sub each, emit+dispatch x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: chained events (handler emits another event)
    {
        let mut bus = EventBus::new();
        bus.on("first", |_| {
            None
        });
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            bus.emit("first", HashMap::new());
            bus.drain_and_dispatch();
        }
        let elapsed = start.elapsed();
        println!(
            "single event dispatch x10000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 10_000.0
        );
    }

    // Benchmark: Transform hook chain
    {
        let mut bus = EventBus::new();
        bus.on("transform-test", |_| None);
        for i in 0..10 {
            bus.add_hook("transform-test", magma::kernel::event::HookPhase::Transform, move |data| {
                let mut d = data.clone();
                d.insert(format!("k{}", i), format!("v{}", i));
                Some(d)
            });
        }
        let start = std::time::Instant::now();
        for _ in 0..1_000 {
            bus.emit("transform-test", HashMap::new());
            bus.drain_and_dispatch();
        }
        let elapsed = start.elapsed();
        println!(
            "10 Transform hooks emit+dispatch x1000: {:?} ({:.1} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Benchmark: subscriber_count on populated bus
    {
        let mut bus = EventBus::new();
        for i in 0..50 {
            bus.on(&format!("evt-{}", i), move |_| {
                let _ = i;
                None
            });
        }
        let start = std::time::Instant::now();
        for _ in 0..100_000 {
            let _ = bus.subscriber_count("evt-0");
        }
        let elapsed = start.elapsed();
        println!(
            "subscriber_count lookup x100000: {:?} ({:.3} µs/op)",
            elapsed,
            elapsed.as_micros() as f64 / 100_000.0
        );
    }
}
