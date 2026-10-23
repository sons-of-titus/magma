# Breakpoint gutter extension — Phase 7.
#
# The :breakpoints gutter column is a Rust GutterProvider reading live from
# ed.debug.breakpoints.  Toggling on gutter click is ALSO native Rust
# (input/mouse.rs → debug/toggle_breakpoint_at): built-in behavior must not
# ride on init-registered Janet event handlers.  These bp/ wrappers remain
# for scripts that drive breakpoints themselves.

(def *breakpoints* @{})   # {buf-key -> set-of-line-numbers}

(defn bp/set [buf line]
  (let [lines (get *breakpoints* buf @{})]
    (put lines line true)
    (put *breakpoints* buf lines))
  (debug/add-breakpoint (or (buffer/path buf) "") line))

(defn bp/clear [buf line]
  (when-let [lines (get *breakpoints* buf)]
    (put lines line nil)
    (when (empty? lines)
      (put *breakpoints* buf nil)))
  (debug/remove-breakpoint (or (buffer/path buf) "") line))

(defn bp/toggle [buf line]
  (if (get-in *breakpoints* [buf line])
    (bp/clear buf line)
    (bp/set buf line)))

(defn bp/list [buf]
  (keys (get *breakpoints* buf @{})))

(command/define "bp/toggle-at-cursor"
  (fn []
    (let [buf (editor/focused-buffer)
          line (buffer/line-number buf)]
      (bp/toggle buf line))))
