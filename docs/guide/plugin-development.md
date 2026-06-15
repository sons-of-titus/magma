# Plugin Development Guide

## Overview

A Magma plugin is simply a `.janet` file. There is no manifest, no build step, no special format. If it's in `~/.magma/plugins/`, it's a plugin.

## Plugin Basics

### Minimal Plugin

```janet
# ~/.magma/plugins/hello.janet
(print "Hello plugin loaded!")
```

### Plugin with Commands

```janet
# ~/.magma/plugins/greeter.janet
(defn greet [& args]
  (def name (or (get args :name) "World"))
  (print "Hello, " name "!"))

(command/define "greet" greet
  {:args [{:name :name :type :string :optional true}]
   :doc "Greet someone"})
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

(command/define "counter-inc" increment {:doc "Increment counter"})
(command/define "counter-reset" reset {:doc "Reset counter"})
```

## Plugin Lifecycle

### Loading

Plugins are loaded automatically from:

```
~/.magma/plugins/
```

Load order is determined by dependency graph (via `require` statements).

### Unloading

Plugins can define a cleanup function:

```janet
# ~/.magma/plugins/cleanup-example.janet
(def- timer-fiber nil)

(defn start []
  (set timer-fiber (fiber/new
    (fn []
      (while true
        (do-something)
        (fiber/sleep 60))))))

(defn stop []
  (when timer-fiber
    (fiber/cancel timer-fiber)))

# Called on reload or plugin removal
(defn on-unload []
  (stop)
  (print "Cleanup example unloaded"))

(start)
```

### Hot Reload

Plugins can be reloaded without restarting the editor:

```janet
# From REPL
(editor/reload "counter")

# Watch for changes and auto-reload
(fs/watch "~/.magma/plugins/counter.janet"
  (fn [path]
    (editor/reload "counter")
    (print "Reloaded: " path)))
```

## API Access

### Buffer Operations

```janet
(defn uppercase-buffer []
  (def buf (buffer/current))
  (def text (buffer/slice buf 0 (buffer/len buf)))
  (buffer/replace buf 0 (buffer/len buf) (string/upper text)))

(command/define "uppercase-buffer" uppercase-buffer
  {:doc "Convert buffer to uppercase"})
```

### Event Subscriptions

```janet
# Track file saves
(event/on :buffer-saved
  (fn [ev]
    (def path (:path ev))
    (def name (buffer/name (:buffer-id ev)))
    (print "Saved " name " to " path)))

# Auto-format on save
(event/on :before-save
  (fn [ev]
    (auto-format (:buffer-id ev))))
```

### Custom Keybindings

```janet
(defn setup-plugin-keybindings []
  (keymap/set "ctrl-e" "plugin-action" :vim)
  (keymap/set "ctrl-e" "plugin-insert" :insert))
```

## Best Practices

### 1. Use Local State

```janet
# GOOD: Local state
(def- state @{})

# BAD: Global state
(def state @{})
```

### 2. Prefix Commands

```janet
# GOOD: Namespaced
(command/define "git-blame" ...)
(command/define "git-log" ...)

# BAD: Generic
(command/define "blame" ...)
(command/define "log" ...)
```

### 3. Handle Errors

```janet
(defn safe-read-file [path]
  (def result (protect (fs/read path)))
  (if (first result)
    (second result)     # Success
    (do
      (print "Error reading: " path)
      nil)))            # Failure
```

### 4. Clean Up Resources

```janet
(def- watcher-id nil)

(defn start-watching []
  (set watcher-id
    (fs/watch "~/project"
      (fn [path] (print "Changed: " path)))))

(defn on-unload []
  (when watcher-id
    (fs/unwatch watcher-id)))
```

### 5. Avoid Blocking

```janet
# GOOD: Spawn a fiber for long work
(event/on :key-pressed
  (fn [ev]
    (fiber/new
      (fn [] (expensive-operation))
      :e 0)))

# BAD: Block the event loop
(event/on :key-pressed
  (fn [ev]
    (expensive-operation)))  # Blocks all input
```

### 6. Document Commands

```janet
(command/define "my-command"
  my-fn
  {:args [{:name :input :type :string :required true}]
   :doc "Does something useful with input"})
```

## Example Plugins

### Git Plugin

```janet
# ~/.magma/plugins/git.janet
(defn git-blame []
  (def buf (buffer/current))
  (def path (buffer/path buf))
  (when path
    (def result (os/execute ["git" "blame" path] :p))
    (def blame-buf (buffer/create "*git-blame*"))
    (buffer/insert blame-buf 0 result)
    (window/set-buffer (window/current) blame-buf)))

(defn git-status []
  (def result (os/execute ["git" "status" "--short"] :p))
  (def status-buf (buffer/create "*git-status*"))
  (buffer/insert status-buf 0 result)
  (window/set-buffer (window/current) status-buf))

(command/define "git-blame" git-blame {:doc "Show git blame"})
(command/define "git-status" git-status {:doc "Show git status"})

(keymap/set "space g b" "git-blame" :vim)
(keymap/set "space g s" "git-status" :vim)
```

### LSP Plugin (Sketch)

```janet
# ~/.magma/plugins/lsp.janet
(def- clients @{})

(defn start-lsp [language]
  (def command (case language
    :rust ["rust-analyzer"]
    :python ["pyright"]
    :javascript ["typescript-language-server" "--stdio"]
    :clojure ["clojure-lsp"]))
  (when command
    (put clients language
      {:process (os/spawn command :pipe)
       :buffer (buffer/current)})))

(defn lsp-format []
  (def buf (buffer/current))
  (def lang (buffer/language buf))
  (def client (get clients lang))
  (when client
    (def result (lsp-request client :textDocument/formatting))
    (when result
      (apply-edits buf result))))

(command/define "lsp-format" lsp-format {:doc "Format buffer via LSP"})

(event/on :buffer-created
  (fn [ev]
    (def buf (:buffer-id ev))
    (def ext (buffer/extension buf))
    (def lang (case ext
      "rs" :rust
      "py" :python
      "js" :javascript
      "ts" :typescript
      nil))
    (when lang
      (start-lsp lang))))
```

### Snippet Plugin (Sketch)

```janet
# ~/.magma/plugins/snippets.janet
(def- snippets
  {:rust {:fn ["fn ${1:name}(${2:args}) {${3:body}}"]
          :impl ["impl ${1:Type} {${2:methods}}"]}
   :clojure {:defn ["(defn ${1:name} [${2:args}]${3:body})"]
             :let ["(let [${1:bindings}]${2:body})"]}})

(defn expand-snippet [snippet-name]
  (def buf (buffer/current))
  (def lang (buffer/language buf))
  (def lang-snippets (get snippets lang))
  (def snippet (get lang-snippets snippet-name))
  (when snippet
    (buffer/insert buf (buffer/cursor buf) (first snippet))))

(command/define "snippet-expand" expand-snippet
  {:args [{:name :name :type :string :required true}]
   :doc "Expand a snippet"})
```

## Testing Plugins

Test plugins directly in the REPL:

```janet
# Load your plugin
> (editor/load "~/.magma/plugins/my-plugin.janet")

# Test commands
> (command/run "my-command")
=> result

# Inspect registrations
> (command/exists? "my-command")
=> true

# Modify and reload
> (editor/reload "my-plugin")
```

## Debugging

```janet
# Print debugging (appears in REPL or status line)
(print "Debug: " some-value)

# Use describe for introspection
(describe (buffer/current))
(describe command/save-buffer)

# Check for errors
(def result (protect (risky-operation)))
(if (first result)
  (print "Success: " (second result))
  (print "Error: " (second result)))
```

## Distribution

Plugins are shared as `.janet` files:

1. Create a `.janet` file
2. Place it in `~/.magma/plugins/`
3. Restart or call `(editor/load "path/to/plugin.janet")`

For sharing, simply distribute the `.janet` file. Users place it in their plugins directory.
