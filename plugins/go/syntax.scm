; Tree-sitter highlight query for go buffers.
;
; Registered automatically by plugin/define as go-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "break" "case" "chan" "const" "continue" "default"
  "defer" "else" "fallthrough" "for" "func" "go"
  "goto" "if" "import" "interface" "map" "package"
  "range" "return" "select" "struct" "switch" "type"
  "var"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_declaration name: (identifier) @function)
(method_declaration name: (field_identifier) @function)
(call_expression function: (identifier) @function)
(call_expression
  function: (selector_expression field: (field_identifier) @function))

; ── Types ─────────────────────────────────────────────────────────────────

(type_spec name: (type_identifier) @type)
(type_identifier) @type

; ── Properties ────────────────────────────────────────────────────────────

(selector_expression field: (field_identifier) @property)
(field_identifier) @property

; ── Constants ─────────────────────────────────────────────────────────────

(int_literal) @constant
(float_literal) @constant
(imaginary_literal) @constant
(true) @constant
(false) @constant
(nil) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(interpreted_string_literal) @string
(raw_string_literal) @string
(rune_literal) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
