; Tree-sitter highlight query for nim buffers.
;
; Registered automatically by plugin/define as nim-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "and" "as" "asm" "bind" "block" "break" "case"
  "cast" "concept" "const" "continue" "converter" "defer"
  "discard" "distinct" "div" "do" "elif" "else" "enum"
  "except" "export" "finally" "for" "from" "func" "if" "import"
  "in" "include" "is" "isnot" "iterator" "let"
  "macro" "method" "mixin" "mod" "not" "notin" "object" "of"
  "or" "out" "proc" "ptr" "raise" "ref" "return" "shl" "shr"
  "static" "template" "try" "tuple" "type" "using" "var"
  "when" "while" "xor" "yield"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(proc_declaration name: (identifier) @function)
(func_declaration name: (identifier) @function)
(iterator_declaration name: (identifier) @function)
(template_declaration name: (identifier) @function)
(macro_declaration name: (identifier) @function)
(call (identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(type_declaration
  (type_symbol_declaration name: (identifier) @type))

; ── Constants ─────────────────────────────────────────────────────────────

(integer_literal) @constant
(float_literal) @constant
(nil_literal) @constant

; ── Strings ───────────────────────────────────────────────────────────────

[
  (interpreted_string_literal)
  (raw_string_literal)
  (long_string_literal)
] @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
(block_comment) @comment
(documentation_comment) @comment
