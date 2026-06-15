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

# Each plugin declares (def *plugin-meta* {:name "..." :version "..." :deps [...]}).
# Call this after loading a plugin to record its metadata and check deps.
(defn plugin/register-meta [meta]
  (def log-buf (buffer/find-or-create "*plugins-log*"))
  (buffer/set-ephemeral log-buf true)
  (def name (or (meta :name) "<unnamed>"))
  (def version (or (meta :version) "0.0.0"))
  (def deps (or (meta :deps) []))
  # Check that declared deps have been loaded
  (each dep deps
    (when (nil? (*loaded-modules* dep))
      (def msg (string "[plugin] " name " requires " dep " (not loaded)\n"))
      (buffer/set-read-only log-buf false)
      (buffer/insert log-buf (buffer/len log-buf) msg)
      (buffer/set-read-only log-buf true)))
  (def msg (string "[plugin] loaded " name " " version "\n"))
  (buffer/set-read-only log-buf false)
  (buffer/insert log-buf (buffer/len log-buf) msg)
  (buffer/set-read-only log-buf true)
  nil)

# Colon verb: show the plugins log buffer
(put *colon-plugins* "plugins-log"
  (fn [_args]
    (def buf (buffer/find-or-create "*plugins-log*"))
    (buffer/set-ephemeral buf true)
    (buffer/set-read-only buf true)
    (editor/run-command "switch-to-buffer" (string (buffer/name buf)))))

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
(keymap/set "ctrl-o" "mark-ring-jump-back" "vim")
(keymap/set "ctrl-i" "mark-ring-jump-forward" "vim")

# ── GPU atlas invalidation ────────────────────────────────────────────────────

# When any font change is declared, mark the atlas dirty so the renderer
# rebuilds it on the next frame.
(event/on "font-changed" (fn [_] (font/invalidate)))
