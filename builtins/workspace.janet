# Workspace Persistence — Phase 8.
#
# Provides colon-mode verbs for saving and restoring workspace state, plus
# event handlers that log workspace lifecycle events to the message area.

# ── Event handlers ────────────────────────────────────────────────────────────

(event/on "workspace-saved"
  (fn [data]
    (let [n (get data :buffer-count "0")]
      (editor/log-message (string "Workspace saved (" n " buffers)")))))

(event/on "workspace-restored"
  (fn [data]
    (let [n (get data :buffer-count "0")]
      (editor/log-message (string "Workspace restored (" n " buffers)")))))

(event/on "workspace-session-saved"
  (fn [data]
    (let [name (get data :name "?")
          n    (get data :buffer-count "0")]
      (editor/log-message (string "Session \"" name "\" saved (" n " buffers)")))))

(event/on "workspace-session-loaded"
  (fn [data]
    (let [name (get data :name "?")
          n    (get data :buffer-count "0")]
      (editor/log-message (string "Session \"" name "\" loaded (" n " buffers)")))))

# ── Helper functions ──────────────────────────────────────────────────────────

(defn workspace/save-and-report
  "Save the workspace and print a brief status message."
  []
  (let [result (workspace/save)]
    (when (= (get result :status) "error")
      (editor/log-message (string "workspace/save error: " (get result :error ""))))))

(defn workspace/restore-and-report
  "Restore the workspace and print a brief status message."
  []
  (let [result (workspace/restore)]
    (when (= (get result :status) "error")
      (editor/log-message (string "workspace/restore error: " (get result :error ""))))))

# ── Colon-mode verbs ──────────────────────────────────────────────────────────

(colon/define "ws-save"
  (fn [_]
    (workspace/save-and-report)))

(colon/define "ws-restore"
  (fn [_]
    (workspace/restore-and-report)))

(colon/define "ws-session-save"
  (fn [name]
    (if (empty? name)
      (editor/log-message "ws-session-save: provide a session name")
      (let [result (workspace/session-save name)]
        (when (= (get result :status) "error")
          (editor/log-message
            (string "workspace/session-save error: " (get result :error ""))))))))

(colon/define "ws-session-load"
  (fn [name]
    (if (empty? name)
      (editor/log-message "ws-session-load: provide a session name")
      (let [result (workspace/session-load name)]
        (when (= (get result :status) "error")
          (editor/log-message
            (string "workspace/session-load error: " (get result :error ""))))))))

(colon/define "ws-sessions"
  (fn [_]
    (let [sessions (workspace/session-list)]
      (if (empty? sessions)
        (editor/log-message "No saved sessions")
        (editor/log-message (string "Sessions: " (string/join sessions ", ")))))))

# ── Command registrations ─────────────────────────────────────────────────────

(command/define "workspace/save"    (fn [] (workspace/save-and-report)))
(command/define "workspace/restore" (fn [] (workspace/restore-and-report)))
