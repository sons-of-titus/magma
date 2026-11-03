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

[
  "data" "dynamic" "for" "for_each" "if"
  "in" "locals" "module" "null" "output"
  "provider" "resource" "terraform" "variable"
] @keyword

; ── Structure ─────────────────────────────────────────────────────────────

(block (identifier) @type (string_lit) @module)
(attribute (identifier) @property)
(object_elem key: (identifier) @property)

; ── Functions ─────────────────────────────────────────────────────────────

(function_call function: (identifier) @function)

; ── Constants ─────────────────────────────────────────────────────────────

(numeric_lit) @constant
"true" @constant
"false" @constant
"null" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_lit) @string
(heredoc) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
