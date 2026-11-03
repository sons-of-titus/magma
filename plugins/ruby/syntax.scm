; Tree-sitter highlight query for ruby buffers.
;
; Registered automatically by plugin/define as ruby-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "__ENCODING__" "__FILE__" "__LINE__" "__callee__" "__dir__"
  "__method__" "alias" "and" "begin" "break" "case" "class"
  "def" "defined?" "do" "else" "elsif" "end" "ensure"
  "for" "if" "in" "module" "next" "nil" "not" "or"
  "redo" "rescue" "retry" "return" "self" "super"
  "then" "undef" "unless" "until" "when" "while" "yield"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(method name: (identifier) @function)
(singleton_method name: (identifier) @function)
(call method: (identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(class name: (constant) @type)
(module name: (constant) @type)
(constant) @type

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant
"true" @constant
"false" @constant
"nil" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(heredoc_body) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
