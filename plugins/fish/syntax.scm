; Tree-sitter highlight query for fish buffers.
;
; Registered automatically by plugin/define as fish-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (variable_name) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(variable_name) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "begin" "case" "else" "end" "for" "function" "if" "in" "not" "or" "and"
  "return" "switch" "while"
] @keyword
(break) @keyword
(continue) @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_definition name: (word) @function)
(command name: (word) @function)

; ── Strings ───────────────────────────────────────────────────────────────

(double_quote_string) @string
(single_quote_string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
