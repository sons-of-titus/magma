; Tree-sitter highlight query for sh buffers.
;
; Registered automatically by plugin/define as sh-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic variable fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(variable_name) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "case" "do" "done" "elif" "else" "esac" "fi"
  "for" "function" "if" "in" "select" "then"
  "until" "while"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_definition name: (word) @function)
(command name: (command_name) @function)

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(raw_string) @string
(ansi_c_string) @string
(heredoc_body) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
