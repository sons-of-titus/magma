# Event System Design

## Architecture

The event system is a central pub/sub bus with serialized dispatch, hook chains, and asynchronous ingestion.

```
                    ┌────────────────────────────┐
                    │        EventBus              │
                    │  ┌──────────────────────┐   │
                    │  │   Subscriber Map     │   │
                    │  │   event_name →       │   │
                    │  │   [Subscription...]   │   │
                    │  └────────┬─────────────┘   │
                    │           │                  │
                    │  ┌────────▼─────────────┐   │
                    │  │   Ingress Queue      │   │
                    │  │   (lock-free MPSC)   │   │
                    │  └────────┬─────────────┘   │
                    │           │                  │
                    │  ┌────────▼─────────────┐   │
                    │  │   Event Serializer   │   │
                    │  │   (single fiber)     │   │
                    │  └────────┬─────────────┘   │
                    │           │                  │
                    │  ┌────────▼─────────────┐   │
                    │  │   Hook Chain         │   │
                    │  │   before → transform  │   │
                    │  │        → after       │   │
                    │  └────────┬─────────────┘   │
                    │           │                  │
                    │  ┌────────▼─────────────┐   │
                    │  │   Subscriber Dispatch│   │
                    │  └──────────────────────┘   │
                    └────────────────────────────┘
```

## Event Flow

1. **Emit**: Any subsystem calls `EventBus::emit(event_name, data)` which pushes to a lock-free MPSC queue
2. **Drain**: A dedicated Janet fiber drains the queue sequentially
3. **Hook chain**: Each event passes through three hook phases:
   - `before` — Pre-processing hooks, can modify event data
   - `transform` — Transformation hooks, can replace event data
   - `after` — Post-processing hooks, read-only
4. **Dispatch**: Subscribers are called with the (possibly transformed) event data
5. **Return**: Subscribers return `(continue? bool)` — `false` stops propagation

## Event Definitions

### Buffer Events

| Event | Data | Description |
|-------|------|-------------|
| `:buffer-created` | `{:buffer-id id :name str}` | A new buffer was created |
| `:buffer-deleted` | `{:buffer-id id :name str}` | A buffer was deleted |
| `:buffer-changed` | `{:buffer-id id}` | Buffer content changed |
| `:buffer-saved` | `{:buffer-id id :path str}` | Buffer was saved to disk |
| `:buffer-filename-changed` | `{:buffer-id id :old str :new str}` | Buffer filename changed |
| `:buffer-mode-changed` | `{:buffer-id id :mode str}` | Buffer major mode changed |
| `:before-save` | `{:buffer-id id}` | Fires before save (can mutate) |
| `:after-save` | `{:buffer-id id :path str}` | Fires after save completes |

### Cursor Events

| Event | Data | Description |
|-------|------|-------------|
| `:cursor-moved` | `{:buffer-id id :window-id id :row n :col n}` | Cursor position changed |
| `:selection-changed` | `{:buffer-id id :start pos :end pos}` | Selection region changed |

### Window Events

| Event | Data | Description |
|-------|------|-------------|
| `:window-created` | `{:window-id id :direction h/v}` | A new window was created |
| `:window-closed` | `{:window-id id}` | A window was closed |
| `:window-focused` | `{:window-id id :buffer-id id}` | A window gained focus |
| `:window-resized` | `{:window-id id :w n :h n}` | A window was resized |

### Input Events

| Event | Data | Description |
|-------|------|-------------|
| `:key-pressed` | `{:key-seq str :window-id id}` | A key sequence was pressed |
| `:keymap-changed` | `{:layer str :key str :cmd str}` | A keybinding was modified |

### Editor Events

| Event | Data | Description |
|-------|------|-------------|
| `:editor-ready` | `{}` | Editor initialization complete |
| `:before-quit` | `{}` | Editor is about to quit (can cancel) |
| `:repl-eval` | `{:code str :result str}` | A REPL evaluation occurred |
| `:plugin-loaded` | `{:name str :path str}` | A plugin was loaded |
| `:plugin-reloaded` | `{:name str :path str}` | A plugin was reloaded |
| `:error` | `{:source str :message str}` | An error occurred somewhere |

### Mode Events

| Event | Data | Description |
|-------|------|-------------|
| `:mode-entered` | `{:mode str :kind major/minor}` | A mode was activated |
| `:mode-exited` | `{:mode str :kind major/minor}` | A mode was deactivated |

## Janet API

### Subscribing

```janet
# Subscribe to an event (persistent)
(event/on :cursor-moved
  (fn [ev]
    (print "Cursor moved to " (:row ev) "," (:col ev))))
=> <subscription:42>

# Subscribe to an event (one-shot)
(event/once :editor-ready
  (fn [&] (print "Editor loaded!")))
=> <subscription:43>
```

### Unsubscribing

```janet
(event/off 42)
=> nil
```

### Emitting

```janet
(event/emit :my-custom-event {:data "hello"})
=> nil
```

### Inspection

```janet
# List all events with subscriber counts
(event/list-events)
=> @{:cursor-moved 3 :key-pressed 5 :buffer-saved 1}

# List subscribers for an event
(event/list-subscribers :cursor-moved)
=> @[42 43 44]
```

## Hooks

Hooks are typed phases attached to events:

```janet
# Before hook (can modify event data)
(hook/add :before-save :trim-whitespace
  (fn [ev]
    (def buf (:buffer-id ev))
    (trim-trailing-whitespace buf)
    ev))  # must return event data

# Transform hook (can replace event data)
(hook/add :key-pressed :remap
  (fn [ev]
    (if (= (:key-seq ev) "ctrl-h")
      (put ev :key-seq "backspace"))
    ev))

# After hook (read-only observation)
(hook/add :after-save :notify
  (fn [ev]
    (print "Saved: " (:path ev))))
```

## Serialization Guarantee

All event handlers run on a single Janet fiber. This provides:

- **No concurrent handler execution** — handlers don't need locks
- **Deterministic ordering** — handlers execute in subscription order
- **No reentrancy** — emitting an event from within a handler queues it for the next tick

For long-running handlers:

```janet
(event/on :key-pressed
  (fn [ev]
    (fiber/new        # Spawn a new fiber for async work
      (fn []
        (expensive-operation ev))
      :e 0)))         # :e = event flag, 0 = initial yield
```

## Rust Implementation

```rust
pub struct EventBus {
    subscribers: HashMap<Symbol, Vec<Subscription>>,
    ingress_queue: MpscQueue<Event>,
    hooks: HashMap<(Symbol, HookPhase), Vec<Hook>>,
    next_id: u64,
}

pub struct Subscription {
    id: u64,
    handler: JanetFunction,
    oneshot: bool,
}

pub enum HookPhase {
    Before,
    Transform,
    After,
}

impl EventBus {
    pub fn emit(&self, name: &str, data: JanetTable) {
        self.ingress_queue.push(Event { name, data });
    }

    pub fn process_next(&mut self) -> Result<()> {
        if let Some(event) = self.ingress_queue.pop() {
            let data = self.run_hooks(HookPhase::Before, &event)?;
            let data = self.run_hooks(HookPhase::Transform, &event)?;
            self.dispatch(&event, &data)?;
            self.run_hooks(HookPhase::After, &event)?;
        }
        Ok(())
    }
}
```
