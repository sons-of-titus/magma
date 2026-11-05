; Tree-sitter highlight query for python buffers.
;
; Registered automatically by plugin/define as python-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "and" "as" "assert" "async" "await"
  "break" "class" "continue" "def" "del"
  "elif" "else" "except" "finally" "for"
  "from" "global" "if" "import" "in"
  "is" "lambda" "nonlocal" "not" "or"
  "pass" "raise" "return" "try" "while"
  "with" "yield"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_definition name: (identifier) @function)
(call function: (identifier) @function)
(call function: (attribute attribute: (identifier) @function))
(decorator (identifier) @function)
(decorator (attribute attribute: (identifier) @function))

; ── Types ─────────────────────────────────────────────────────────────────

(class_definition name: (identifier) @type)

; ── Attributes / properties ───────────────────────────────────────────────

(attribute attribute: (identifier) @property)

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant
(true) @constant
(false) @constant
(none) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
