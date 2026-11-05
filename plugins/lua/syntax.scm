; Tree-sitter highlight query for lua buffers.
;
; Registered automatically by plugin/define as lua-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "and" "do" "else" "elseif" "end"
  "for" "function" "goto" "if" "in"
  "local" "not" "or" "repeat" "return"
  "then" "until" "while"
] @keyword
(break_statement) @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_declaration name: (identifier) @function)
(function_declaration name: (dot_index_expression) @function)
(function_call name: (identifier) @function)
(method_index_expression method: (identifier) @function)

; ── Properties ────────────────────────────────────────────────────────────

(dot_index_expression field: (identifier) @property)

; ── Constants ─────────────────────────────────────────────────────────────

(number) @constant
(true) @constant
(false) @constant
(nil) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
