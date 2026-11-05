; Tree-sitter highlight query for nix buffers.
;
; Registered automatically by plugin/define as nix-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "assert" "else" "if" "in" "inherit" "let"
  "or" "rec" "then" "with"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(apply_expression function: (variable_expression (identifier) @function))

; ── Attributes ────────────────────────────────────────────────────────────

(attrpath (identifier) @property)

; ── Constants ─────────────────────────────────────────────────────────────

(integer_expression) @constant
(float_expression) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_expression) @string
(indented_string_expression) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
