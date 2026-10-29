# Magma plugin ecosystem: module loader, plugin metadata, mark-ring navigation.

# ── Module loader ─────────────────────────────────────────────────────────────

# Tracks which module names have already been evaluated.
(def *loaded-modules* (table/new 16))

# Load a Magma plugin by name.
# Searches module/path for <name>.janet and evaluates it once.
(defn magma-require [name]
  (when (nil? (*loaded-modules* name))
    (def paths (module/path))
    (var found false)
    (each dir paths
      (def filepath (string dir "/" name ".janet"))
      (when (project/path-exists? filepath)
        (editor/load-file filepath)
        (put *loaded-modules* name true)
        (set found true)
        (break)))
    (unless found
      (editor/warn (string "magma-require: module not found: " name))))
  nil)

# ── Plugin metadata ───────────────────────────────────────────────────────────
# Plugin manifests and dependency resolution live in builtins/plugin_system.janet.

# ── Mark ring key bindings ────────────────────────────────────────────────────

# Forward position ring for ctrl-i (jump forward after ctrl-o).
(def *mark-forward-ring* @[])

(command/define "mark-ring-jump-back"
  (fn [&]
    (def cur-buf (buffer/current))
    (def cur-path (if cur-buf (buffer/path cur-buf) ""))
    (def cur-offset (if cur-buf (buffer/cursor cur-buf) 0))
    (def entry (mark-ring/pop))
    (when entry
      # Save current position to the forward ring before jumping
      (array/push *mark-forward-ring* {:path cur-path :offset cur-offset})
      (def target-path (entry :path))
      (def target-offset (entry :offset))
      (if (= target-path cur-path)
        # Same buffer — just move cursor
        (when cur-buf (buffer/set-cursor cur-buf target-offset))
        # Different file — open it
        (do
          (editor/run-command "open-file" target-path)
          (def new-buf (buffer/current))
          (when new-buf (buffer/set-cursor new-buf target-offset)))))))

(command/define "mark-ring-jump-forward"
  (fn [&]
    (when (> (length *mark-forward-ring*) 0)
      (def entry (array/pop *mark-forward-ring*))
      (def cur-buf (buffer/current))
      (def cur-path (if cur-buf (buffer/path cur-buf) ""))
      (def cur-offset (if cur-buf (buffer/cursor cur-buf) 0))
      # Save current position back to the mark ring
      (mark-ring/push cur-path cur-offset)
      (def target-path (entry :path))
      (def target-offset (entry :offset))
      (if (= target-path cur-path)
        (when cur-buf (buffer/set-cursor cur-buf target-offset))
        (do
          (editor/run-command "open-file" target-path)
          (def new-buf (buffer/current))
          (when new-buf (buffer/set-cursor new-buf target-offset)))))))

# Bind ctrl-o (jump back) and ctrl-i (jump forward) in normal mode
(keymap/set "ctrl-o" "mark-ring-jump-back" "editor")
(keymap/set "ctrl-i" "mark-ring-jump-forward" "editor")

# ── GPU atlas invalidation ────────────────────────────────────────────────────

# When any font change is declared, mark the atlas dirty so the renderer
# rebuilds it on the next frame.
(event/on "font-changed" (fn [_] (font/invalidate)))
