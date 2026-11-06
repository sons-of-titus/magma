//! Central event bus for Magma.
//!
//! Events are queued synchronously and dispatched via `drain_and_dispatch`,
//! which processes the entire pending queue in order.  Hook chains run
//! Before → Transform → After around each subscriber group.

#[macro_use]
extern crate logging;

use std::collections::{HashMap, VecDeque};

pub mod keys;
pub mod payload;

pub type EventData = HashMap<String, String>;

#[derive(Debug, Clone)]
pub struct Event {
    pub name: String,
    pub data: EventData,
    /// How many handler-emitted hops separate this event from a direct
    /// `emit`.  Bounded by [`MAX_CASCADE`] so two handlers emitting each
    /// other's events terminate instead of livelocking the drain loop.
    cascade: u32,
}

/// A→B→A handler cycles die here; real handler chains are 2–4 hops deep.
const MAX_CASCADE: u32 = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum HookPhase {
    Before,
    Transform,
    After,
}

type HandlerFn = Box<dyn Fn(&EventData) -> Option<EventData> + Send + Sync>;

struct Subscription {
    id: u64,
    handler: HandlerFn,
    oneshot: bool,
}

struct Hook {
    id: u64,
    event_name: String,
    handler: HandlerFn,
    phase: HookPhase,
}

pub struct EventBus {
    subscribers: HashMap<String, Vec<Subscription>>,
    hooks: Vec<Hook>,
    pending: VecDeque<Event>,
    next_sub_id: u64,
    next_hook_id: u64,
    /// Names (with their cascade depth) currently being dispatched.  A
    /// handler that re-emits the event it is handling would otherwise grow
    /// the queue forever — the drain loop pops one and the handler pushes
    /// one back, endlessly, on the UI thread.  Re-entrant emits of an
    /// in-flight name are dropped, and cascades are depth-capped.
    dispatching: Vec<(String, u32)>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            subscribers: HashMap::new(),
            hooks: Vec::new(),
            pending: VecDeque::new(),
            next_sub_id: 1,
            next_hook_id: 1,
            dispatching: Vec::new(),
        }
    }

    pub fn on<F>(&mut self, event_name: &str, handler: F) -> u64
    where
        F: Fn(&EventData) -> Option<EventData> + Send + Sync + 'static,
    {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        self.subscribers
            .entry(event_name.to_string())
            .or_default()
            .push(Subscription {
                id,
                handler: Box::new(handler),
                oneshot: false,
            });
        id
    }

    pub fn once<F>(&mut self, event_name: &str, handler: F) -> u64
    where
        F: Fn(&EventData) -> Option<EventData> + Send + Sync + 'static,
    {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        self.subscribers
            .entry(event_name.to_string())
            .or_default()
            .push(Subscription {
                id,
                handler: Box::new(handler),
                oneshot: true,
            });
        id
    }

    pub fn off(&mut self, id: u64) {
        for subs in self.subscribers.values_mut() {
            subs.retain(|s| s.id != id);
        }
        self.hooks.retain(|h| h.id != id);
    }

    /// Queue an event for later dispatch via `drain_and_dispatch`.
    /// An emit of an event name that is already being dispatched is
    /// dropped: it can only be a handler's self-recursion.  An emit that
    /// would exceed [`MAX_CASCADE`] handler hops is also dropped: mutual
    /// handler emission must terminate.
    pub fn emit(&mut self, event_name: &str, data: EventData) {
        if event_name.ends_with("-error") || event_name.ends_with("-failed") || event_name == "error" {
            error!("event '{}': {}", event_name, data.get(keys::keys::ERROR).map(String::as_str).unwrap_or("(no message)"));
        }
        if self.dispatching.iter().any(|(n, _)| n == event_name) {
            debug!("event bus: dropped re-entrant emit of '{}'", event_name);
            return;
        }
        let cascade = self
            .dispatching
            .last()
            .map(|(_, c)| c + 1)
            .unwrap_or(0);
        if cascade > MAX_CASCADE {
            debug!(
                "event bus: dropped '{}': cascade exceeded {} handler hops",
                event_name, MAX_CASCADE
            );
            return;
        }
        self.pending.push_back(Event {
            name: event_name.to_string(),
            data,
            cascade,
        });
    }

    /// Queue an event from a typed payload that implements `Into<EventData>`.
    pub fn emit_typed<T: Into<EventData>>(&mut self, event_name: &str, data: T) {
        self.emit(event_name, data.into());
    }

    /// Dispatch all queued events in order.  Any events emitted by handlers
    /// during dispatch are appended to the queue and processed in the same call.
    pub fn drain_and_dispatch(&mut self) {
        let count = self.pending.len();
        if count > 0 {
            debug!("DD: draining {} events", count);
        }
        while let Some(event) = self.pending.pop_front() {
            debug!("DD: event={}", event.name);
            self.dispatch_all(&event);
            debug!("DD: event={} done", event.name);
        }
    }

    pub fn add_hook<F>(&mut self, event_name: &str, phase: HookPhase, handler: F) -> u64
    where
        F: Fn(&EventData) -> Option<EventData> + Send + Sync + 'static,
    {
        let id = self.next_hook_id;
        self.next_hook_id += 1;
        self.hooks.push(Hook {
            id,
            event_name: event_name.to_string(),
            handler: Box::new(handler),
            phase,
        });
        id
    }

    pub fn dispatch_all(&mut self, event: &Event) {
        debug!(
            "DA: event={}, hooks_before={}",
            event.name,
            self.hooks.len()
        );
        self.dispatching.push((event.name.clone(), event.cascade));
        let data = self
            .run_hooks(&event.name, HookPhase::Before, &event.data)
            .unwrap_or_else(|| event.data.clone());

        let data = self
            .run_hooks(&event.name, HookPhase::Transform, &data)
            .unwrap_or(data);

        if let Some(subs) = self.subscribers.get(&event.name) {
            debug!("DA: event={}, subscriber_count={}", event.name, subs.len());
            let mut to_remove = Vec::new();
            for (i, sub) in subs.iter().enumerate() {
                debug!("DA: calling subscriber[{}]", i);
                let _ = (sub.handler)(&data);
                debug!("DA: subscriber[{}] returned", i);
                if sub.oneshot {
                    to_remove.push(sub.id);
                }
            }
            if !to_remove.is_empty()
                && let Some(subs) = self.subscribers.get_mut(&event.name)
            {
                subs.retain(|s| !to_remove.contains(&s.id));
            }
        }

        self.run_hooks(&event.name, HookPhase::After, &data);
        self.dispatching.pop();
        debug!("DA: event={} complete", event.name);
    }

    fn run_hooks(
        &mut self,
        event_name: &str,
        phase: HookPhase,
        data: &EventData,
    ) -> Option<EventData> {
        let mut current = data.clone();
        for hook in &self.hooks {
            if hook.phase == phase
                && hook.event_name == event_name
                && let Some(new_data) = (hook.handler)(&current)
            {
                current = new_data;
            }
        }
        Some(current)
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    pub fn subscriber_count(&self, event_name: &str) -> usize {
        self.subscribers.get(event_name).map_or(0, |s| s.len())
    }

    pub fn list_subscribers(&self, event_name: &str) -> Vec<u64> {
        self.subscribers
            .get(event_name)
            .map(|subs| subs.iter().map(|s| s.id).collect())
            .unwrap_or_default()
    }

    pub fn list_events(&self) -> Vec<(String, usize)> {
        self.subscribers
            .iter()
            .map(|(name, subs)| (name.clone(), subs.len()))
            .collect()
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_sub_id;
        self.next_sub_id += 1;
        id
    }
}

#[cfg(test)]
mod tests;
