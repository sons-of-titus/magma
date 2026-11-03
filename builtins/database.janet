# Database tool — SQL console commands.
#
# The console buffers are Rust-created (`*sql: <source>*`); this file owns
# the user-facing verbs and the run chord.  Cmd+Enter (meta-return) runs
# the statement under the cursor inside a SQL console and falls back to
# code actions everywhere else, so the chord never goes dead.

(command/define "sql-run"
  (fn [&]
    (def b (buffer/current))
    (if (and b (string/has-prefix? "*sql" (buffer/name b)))
      (editor/run-command "db-execute-current")
      (editor/run-command "lsp-code-actions"))))

(command/define "sql-run-script"
  (fn [&]
    (def b (buffer/current))
    (when (and b (string/has-prefix? "*sql" (buffer/name b)))
      (editor/run-command "db-execute-buffer"))))

(command/define "sql-run-selection"
  (fn [&]
    (def b (buffer/current))
    (when (and b (string/has-prefix? "*sql" (buffer/name b)))
      (editor/run-command "db-execute-selection"))))

(keymap/set "meta-return"  "sql-run"          :editor)
(keymap/set "ctrl-return"  "sql-run"          :editor)
(keymap/set "meta-shift-return" "sql-run-script" :editor)
(keymap/set "ctrl-shift-return" "sql-run-script" :editor)
