# Breakpoint gutter extension — Phase 7.
#
# The :breakpoints gutter column is now a Rust GutterProvider that reads
# live from ed.debug.breakpoints.  This file only provides the Janet-level
# convenience wrappers and the toggle command.

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

(event/on "gutter-clicked"
  (fn [data]
    (when (= (get data :column) ":breakpoints")
      (bp/toggle
        (scan-number (get data :buf))
        (scan-number (get data :line))))))
