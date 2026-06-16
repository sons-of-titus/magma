use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::kernel::event::{EventBus, HookPhase};

fn bus() -> EventBus { EventBus::new() }

#[test]
fn subscriber_receives_emitted_event() {
    let mut b = bus();
    let received = Arc::new(Mutex::new(false));
    let r = received.clone();
    b.on("test-event", move |_data| {
        *r.lock().unwrap() = true;
        None
    });
    b.emit("test-event", HashMap::new());
    b.drain_and_dispatch();
    assert!(*received.lock().unwrap());
}

#[test]
fn subscriber_does_not_receive_different_event() {
    let mut b = bus();
    let received = Arc::new(Mutex::new(false));
    let r = received.clone();
    b.on("other-event", move |_| { *r.lock().unwrap() = true; None });
    b.emit("test-event", HashMap::new());
    b.drain_and_dispatch();
    assert!(!*received.lock().unwrap());
}

#[test]
fn oneshot_fires_once_then_is_removed() {
    let mut b = bus();
    let count = Arc::new(Mutex::new(0u32));
    let c = count.clone();
    b.once("tick", move |_| { *c.lock().unwrap() += 1; None });

    b.emit("tick", HashMap::new());
    b.drain_and_dispatch();
    assert_eq!(*count.lock().unwrap(), 1);

    b.emit("tick", HashMap::new());
    b.drain_and_dispatch();
    assert_eq!(*count.lock().unwrap(), 1); // not fired again
}

#[test]
fn subscriber_count_matches() {
    let mut b = bus();
    assert_eq!(b.subscriber_count("evt"), 0);
    b.on("evt", |_| None);
    b.on("evt", |_| None);
    assert_eq!(b.subscriber_count("evt"), 2);
}

#[test]
fn off_removes_subscription() {
    let mut b = bus();
    let id = b.on("evt", |_| None);
    assert_eq!(b.subscriber_count("evt"), 1);
    b.off(id);
    assert_eq!(b.subscriber_count("evt"), 0);
}

#[test]
fn hook_runs_only_for_matching_event() {
    let mut b = bus();
    let fired = Arc::new(Mutex::new(false));
    let f = fired.clone();
    b.add_hook("specific-event", HookPhase::Before, move |_| {
        *f.lock().unwrap() = true;
        None
    });

    b.emit("other-event", HashMap::new());
    b.drain_and_dispatch();
    assert!(!*fired.lock().unwrap());

    b.emit("specific-event", HashMap::new());
    b.drain_and_dispatch();
    assert!(*fired.lock().unwrap());
}

#[test]
fn hook_transform_phase_can_modify_data() {
    let mut b = bus();
    let received_val = Arc::new(Mutex::new(String::new()));
    let rv = received_val.clone();

    b.add_hook("ev", HookPhase::Transform, |data| {
        let mut out = data.clone();
        out.insert("added".into(), "yes".into());
        Some(out)
    });
    b.on("ev", move |data| {
        *rv.lock().unwrap() = data.get("added").cloned().unwrap_or_default();
        None
    });

    b.emit("ev", HashMap::new());
    b.drain_and_dispatch();
    assert_eq!(&*received_val.lock().unwrap(), "yes");
}

#[test]
fn event_data_passes_through_correctly() {
    let mut b = bus();
    let val = Arc::new(Mutex::new(String::new()));
    let v = val.clone();
    b.on("greet", move |data| {
        *v.lock().unwrap() = data.get("name").cloned().unwrap_or_default();
        None
    });

    let mut data = HashMap::new();
    data.insert("name".into(), "Magma".into());
    b.emit("greet", data);
    b.drain_and_dispatch();
    assert_eq!(&*val.lock().unwrap(), "Magma");
}
