# Debug System — Janet-side integration (Phase 6).
#
# Provides:
#   - Event handlers for debug-stopped, debug-output, debug-session-ended,
#     debug-evaluate-result, and debug-breakpoint-changed.
#   - Commands for common debug operations (run-command / keybindings).
#   - Helper functions that wrap the Rust debug/ API.

# ── Event handlers ────────────────────────────────────────────────────────────
# Adapter output and evaluate results surface in the debug pane's console;
# only lifecycle transitions land in the status message area.

(event/on "debug-stopped"
  (fn [data]
    (let [reason (get data :reason "breakpoint")]
      (editor/log-message (string "Debug: stopped — " reason)))))

(event/on "debug-session-ended"
  (fn [data]
    (editor/log-message
      (string "Debug session " (get data :session-id "") " ended"))))

(event/on "debug-breakpoint-changed"
  (fn [data]
    (let [action (get data :action "add")
          file   (get data :file "")
          line   (get data :line "0")]
      (when (not (empty? file))
        (editor/log-message
          (string (case action
                    "add" "Breakpoint set"
                    "remove" "Breakpoint cleared"
                    "enable" "Breakpoint enabled"
                    "Breakpoint disabled")
                  " at " file ":" line))))))

# ── Helper functions ──────────────────────────────────────────────────────────

(defn debug/toggle-breakpoint
  "Toggle a breakpoint at the cursor position in the focused buffer."
  []
  (let [file (buffer/path (editor/focused-buffer))
        # Breakpoints are stored 1-based (DAP convention); buffer/line-number is 0-based.
        line (+ 1 (buffer/line-number (editor/focused-buffer)))]
    (when file
      (let [bps (debug/breakpoints file)]
        (if (find |(= $ line) bps)
          (debug/remove-breakpoint file line)
          (debug/add-breakpoint file line))))))

(defn debug/session?
  "Return true if a debug session is active."
  []
  (not (nil? (debug/session))))

# ── Commands ─────────────────────────────────────────────────────────────────

# Launching needs a run configuration's program/args/cwd; `debug-config`
# builds the DAP launch request from it.  An explicit adapter overrides
# detection — otherwise the adapter is chosen from the detected language.
(command/define "dbg-start"
  (fn [arg]
    (if (empty? arg)
      (command/run "debug-config")
      (command/run "debug-config" "adapter" arg))))

(command/define "dbg-bp"
  (fn [_]
    (debug/toggle-breakpoint)))

(command/define "dbg-continue"
  (fn [_]
    (debug/continue)))

(command/define "dbg-step-in"
  (fn [_]
    (debug/step-in)))

(command/define "dbg-step-over"
  (fn [_]
    (debug/step-over)))

(command/define "dbg-step-out"
  (fn [_]
    (debug/step-out)))

(command/define "dbg-pause"
  (fn [_]
    (debug/pause)))

(command/define "dbg-restart"
  (fn [_]
    (debug/restart)))

(command/define "dbg-stop"
  (fn [_]
    (debug/disconnect)))

(command/define "dbg-watch-add"
  (fn [expr]
    (when (not (empty? expr))
      (debug/watch-add expr))))

(command/define "dbg-watch-remove"
  (fn [expr]
    (when (not (empty? expr))
      (debug/watch-remove expr))))

(command/define "dbg-eval"
  (fn [expr]
    (when (not (empty? expr))
      (debug/evaluate expr))))

# Register the toggle-breakpoint command so it can be bound to a key.
(command/define "debug/toggle-breakpoint"
  (fn [] (debug/toggle-breakpoint)))
