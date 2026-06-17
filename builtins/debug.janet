# Debug System — Janet-side integration (Phase 6).
#
# Provides:
#   - Event handlers for debug-stopped, debug-output, debug-session-ended,
#     debug-evaluate-result, and debug-breakpoint-changed.
#   - Colon-mode verbs for common debug operations.
#   - Helper functions that wrap the Rust debug/ API.

# ── Event handlers ────────────────────────────────────────────────────────────

(event/on "debug-stopped"
  (fn [data]
    (let [reason (get data :reason "breakpoint")]
      (editor/log-message (string "Debug: stopped — " reason)))))

(event/on "debug-output"
  (fn [data]
    (let [cat (get data :category "console")
          out (string/trimr (get data :output ""))]
      (when (and (not (empty? out)) (= cat "console"))
        (editor/log-message out)))))

(event/on "debug-session-ended"
  (fn [data]
    (editor/log-message
      (string "Debug session " (get data :session-id "") " ended"))))

(event/on "debug-evaluate-result"
  (fn [data]
    (editor/log-message (string "= " (get data :result "")))))

(event/on "debug-breakpoint-changed"
  (fn [data]
    (let [action (get data :action "add")
          file   (get data :file "")
          line   (get data :line "0")]
      (when (not (empty? file))
        (editor/log-message
          (string (if (= action "add") "Breakpoint set" "Breakpoint cleared")
                  " at " file ":" line))))))

# ── Helper functions ──────────────────────────────────────────────────────────

(defn debug/toggle-breakpoint
  "Toggle a breakpoint at the cursor position in the focused buffer."
  []
  (let [file (buffer/path (editor/focused-buffer))
        line (buffer/line-number (editor/focused-buffer))]
    (when file
      (let [bps (debug/breakpoints file)]
        (if (find |(= $ line) bps)
          (debug/remove-breakpoint file line)
          (debug/add-breakpoint file line))))))

(defn debug/session?
  "Return true if a debug session is active."
  []
  (not (nil? (debug/session))))

# ── Colon verbs ───────────────────────────────────────────────────────────────

(colon/define "dbg-start"
  (fn [arg]
    (debug/start (if (empty? arg) "codelldb" arg))))

(colon/define "dbg-bp"
  (fn [_]
    (debug/toggle-breakpoint)))

(colon/define "dbg-continue"
  (fn [_]
    (debug/continue)))

(colon/define "dbg-step-in"
  (fn [_]
    (debug/step-in)))

(colon/define "dbg-step-over"
  (fn [_]
    (debug/step-over)))

(colon/define "dbg-step-out"
  (fn [_]
    (debug/step-out)))

(colon/define "dbg-eval"
  (fn [expr]
    (when (not (empty? expr))
      (debug/evaluate expr))))

# Register the toggle-breakpoint command so it can be bound to a key.
(command/define "debug/toggle-breakpoint"
  (fn [] (debug/toggle-breakpoint)))
