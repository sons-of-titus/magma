# Plugin System Design

## Overview

Plugins are just Janet code. There is no special plugin format, no manifest files, no build step. A plugin is a `.janet` file placed in `~/.magma/plugins/`.

## Plugin Lifecycle

```
                    ┌─────────────────────────────┐
                    │    Plugin Loading Pipeline    │
                    │                              │
                    │  ┌───────────────────────┐  │
                    │  │ 1. Discovery           │  │
                    │  │   Scan plugins/ dir    │  │
                    │  │   Build file list      │  │
                    │  └───────────┬───────────┘  │
                    │              ▼              │
                    │  ┌───────────────────────┐  │
                    │  │ 2. Parse & Dep        │  │
                    │  │    Resolution          │  │
                    │  │   Topological sort     │  │
                    │  │   Cycle detection      │  │
                    │  └───────────┬───────────┘  │
                    │              ▼              │
                    │  ┌───────────────────────┐  │
                    │  │ 3. Sandboxed Eval      │  │
                    │  │   Fresh environment    │  │
                    │  │   Capture exports      │  │
                    │  │   Timeout: 5s          │  │
                    │  └───────────┬───────────┘  │
                    │              ▼              │
                    │  ┌───────────────────────┐  │
                    │  │ 4. Registration        │  │
                    │  │   Commands → Registry  │  │
                    │  │   Hooks → Event Bus    │  │
                    │  │   Keymaps → Manager    │  │
                    │  └───────────┬───────────┘  │
                    │              ▼              │
                    │  ┌───────────────────────┐  │
                    │  │ 5. Caching             │  │
                    │  │   PluginRecord stored  │  │
                    │  │   in PluginRegistry    │  │
                    │  └───────────────────────┘  │
                    │                              │
                    │  Hot Reload Path:            │
                    │  5a. Call unload hook        │
                    │  5b. Remove registrations    │
                    │  5c. Eval new file           │
                    │  5d. Re-register             │
                    └─────────────────────────────┘
```

## Plugin Structure

### Simple Plugin

```janet
# ~/.magma/plugins/hello.janet
(defn say-hello []
  (print "Hello from plugin!"))

(command/define "hello" say-hello
  {:doc "Print a greeting"})

(event/on :editor-ready
  (fn [&] (print "Hello plugin loaded!")))
```

### Plugin with State

```janet
# ~/.magma/plugins/counter.janet
(def- state @{:count 0})

(defn increment []
  (put state :count (+ (state :count) 1))
  (print "Count: " (state :count)))

(defn reset []
  (put state :count 0)
  (print "Reset"))

(command/define "counter-increment" increment
  {:doc "Increment the counter"})
(command/define "counter-reset" reset
  {:doc "Reset the counter"})
```

### Plugin with Unload Hook

```janet
# ~/.magma/plugins/timer.janet
(def- fiber nil)

(defn start-timer []
  (set fiber (fiber/new
    (fn []
      (while true
        (print "tick")
        (fiber/sleep 1)))))

(defn stop-timer []
  (when fiber
    (fiber/cancel fiber)))

# Register unload hook for clean teardown
(defn on-unload []
  (stop-timer)
  (print "Timer plugin unloaded"))

(command/define "timer-start" start-timer
  {:doc "Start the timer"})
(command/define "timer-stop" stop-timer
  {:doc "Stop the timer"})
```

## Plugin Registry

```rust
pub struct PluginRegistry {
    plugins: HashMap<String, PluginRecord>,
    load_order: Vec<String>,
}

pub struct PluginRecord {
    pub name: String,
    pub path: PathBuf,
    pub source: String,                           // Original source code
    pub environment: Option<JanetEnvironment>,     // Plugin's sandbox
    pub exports: HashMap<String, JanetFunction>,
    pub commands: Vec<CommandId>,
    pub hooks: Vec<HookId>,
    pub keybindings: Vec<(String, String)>,        // (key-seq, command-name)
    pub subscriptions: Vec<SubscriptionId>,
    pub unload_hook: Option<JanetFunction>,
}
```

## Dependency Management

Plugins can depend on other plugins via Janet's `require`:

```janet
# ~/.magma/plugins/lsp.janet
(require plugins/git)  # Depends on git plugin

(defn lsp-attach [buf-id]
  (git/blame buf-id)  # Use git plugin's function
  ...)
```

The loader resolves dependencies topologically:

1. Parse all `require` calls from each plugin
2. Build dependency graph
3. Detect cycles (error if found)
4. Load in dependency order

## Hot Reload

### From REPL

```janet
# Reload a specific plugin
(editor/reload "my-plugin")

# Reload all plugins
(each plugin (plugin/list)
  (editor/reload plugin))
```

### Automatic Reload

```janet
# Watch plugin file and auto-reload
(fs/watch "~/.magma/plugins/my-plugin.janet"
  (fn [path]
    (editor/reload "my-plugin")
    (print "Reloaded: " path)))
```

### Reload Process

1. Look up `PluginRecord` for the named plugin
2. Call `unload_hook` if defined (allows cleanup)
3. Remove all registered commands from the registry
4. Remove all hooks from the event bus
5. Remove all keybindings from the keymap manager
6. Remove all event subscriptions
7. Re-evaluate the plugin source in a fresh environment
8. Re-register all commands, hooks, keybindings, subscriptions
9. Update the `PluginRecord`
10. Emit `:plugin-reloaded` event

## Plugin Loading Order

1. **Built-in plugins** — Embedded in the Rust binary:
   - `builtins/init.janet` — Core initialization, REPL setup
   - `builtins/vim.janet` — Vim keybindings (default mode)

2. **User init** — `~/.magma/init.janet`

3. **Plugin directory** — `~/.magma/plugins/*.janet` (sorted by dependency)

4. **User override** — `~/.magma/user.janet` (loaded last, for overrides)

## Sandboxing

Each plugin gets a fresh environment:

```janet
# Plugin A
(def !secret "password")

# Plugin B (separate environment, cannot access !secret)
(print !secret)  # => nil
```

The sandbox restricts:

- No access to `os/shell` or `os/execute` by default (can be enabled via `dangerous-permissions`)
- No access to `ffi` by default
- Access to editor APIs is controlled (cannot directly mutate Rust state)
- File system access is limited to `~/.magma/` directory by default

## Best Practices

1. **Use `def-` for private state** — prefix with `-` to indicate module-private
2. **Define an unload hook** — for cleanup for timers, fibers, file watchers, etc.
3. **Prefix commands** — use namespaced names like `git-blame`, `lsp-format`
4. **Handle errors** — use `protect` for operations that might fail
5. **Avoid global state** — prefer local state scoped to the plugin
