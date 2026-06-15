# Grep and navigation: ripgrep quickfix integration + recent-files picker.

# ── Ripgrep / quickfix ────────────────────────────────────────────────────────

# Parse a single ripgrep line "file:line:col:message" into a quickfix entry table.
(defn- grep/parse-line [line]
  (def parts (string/split ":" line))
  (when (>= (length parts) 4)
    (def filename (parts 0))
    (def ln (scan-number (parts 1)))
    (def col (scan-number (parts 2)))
    (def msg (string/join (array/slice parts 3) ":"))
    (when (and filename ln col msg)
      {:filename filename :line ln :col col :message (string/trim msg)})))

# Run ripgrep with `pattern` (and optional `path`) and populate the quickfix list.
(defn grep/run [pattern &opt path]
  (def search-path (or path "."))
  (def output-buf (buffer/find-or-create "*grep*"))
  (buffer/set-ephemeral output-buf true)
  (buffer/set-read-only output-buf false)
  (buffer/delete output-buf 0 (buffer/len output-buf))
  (buffer/insert output-buf 0 (string "rg --line-number --column " pattern "\n\n"))
  (def entries @[])
  # Capture process output lines via event handlers
  (var proc-id nil)
  (event/once "process-exit"
    (fn [payload]
      (when (= (payload :id) proc-id)
        (quickfix/set entries)
        (buffer/set-read-only output-buf true)
        (def n (length entries))
        (editor/log-message (string "grep: " n " matches for " pattern)))))
  (event/on "process-output"
    (fn [payload]
      (when (= (payload :id) proc-id)
        (def line (string/trim (payload :line)))
        (when (not= line "")
          (buffer/set-read-only output-buf false)
          (buffer/insert output-buf (buffer/len output-buf) (string line "\n"))
          (buffer/set-read-only output-buf true)
          (def entry (grep/parse-line line))
          (when entry (array/push entries entry))))))
  (set proc-id (process/spawn (string "rg --line-number --column " pattern " " search-path) [])))

# Colon verb: :grep <pattern>
(put *colon-plugins* "grep"
  (fn [args]
    (def pattern (string/trim args))
    (if (= pattern "")
      (editor/log-message "Usage: :grep <pattern>")
      (grep/run pattern))))

# ── Recent-files picker ───────────────────────────────────────────────────────

(defn recent-files/show []
  (def recent (project/recent))
  (def buf (buffer/find-or-create "*recent-files*"))
  (buffer/set-ephemeral buf true)
  (buffer/set-read-only buf false)
  (buffer/delete buf 0 (buffer/len buf))
  (if (= (length recent) 0)
    (buffer/insert buf 0 "  (no recent projects/files)\n")
    (each path recent
      (buffer/insert buf (buffer/len buf) (string "  " path "\n"))))
  (buffer/set-read-only buf true)
  (editor/run-command "switch-to-buffer" (buffer/name buf)))

(command/define "recent-files"
  (fn [& _]
    (recent-files/show)))

# ctrl-r in normal mode opens the recent-files picker
(keymap/set "ctrl-r" "recent-files" "vim")

# Colon verb: :recent
(put *colon-plugins* "recent"
  (fn [_args]
    (recent-files/show)))
