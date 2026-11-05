; Tree-sitter highlight query for javascript buffers.
;
; Registered automatically by plugin/define as javascript-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "async" "await" "break" "case" "catch" "class" "const"
  "continue" "debugger" "default" "delete" "do" "else"
  "export" "extends" "finally" "for" "from" "function"
  "if" "import" "in" "instanceof" "let" "new"
  "of" "return" "static" "switch" "target"
  "throw" "try" "typeof" "var" "void" "while"
  "with" "yield"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_declaration name: (identifier) @function)
(function_expression name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function)
(call_expression
  function: (member_expression property: (property_identifier) @function))

; ── Types ─────────────────────────────────────────────────────────────────

(class_declaration name: (identifier) @type)
(class name: (identifier) @type)

; ── Properties ────────────────────────────────────────────────────────────

(member_expression property: (property_identifier) @property)

; ── Constants ─────────────────────────────────────────────────────────────

(number) @constant
(true) @constant
(false) @constant
(null) @constant
(undefined) @constant
(this) @variable
(super) @variable

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(template_string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
