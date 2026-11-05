; Tree-sitter highlight query for terraform buffers.
;
; Registered automatically by plugin/define as terraform-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

["for" "in" "if"] @keyword

; ── Structure ─────────────────────────────────────────────────────────────

(block (identifier) @type (string_lit) @module)
(attribute (identifier) @property)
(object_elem key: (expression (variable_expr (identifier) @property)))

; ── Functions ─────────────────────────────────────────────────────────────

(function_call (identifier) @function)

; ── Constants ─────────────────────────────────────────────────────────────

(numeric_lit) @constant
"true" @constant
"false" @constant
(null_lit) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_lit) @string
(heredoc_template) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
