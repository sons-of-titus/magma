; Tree-sitter highlight query for kotlin buffers.
;
; Registered automatically by plugin/define as kotlin-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic identifier fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(simple_identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "abstract" "actual" "as" "break" "by" "catch" "class" "companion"
  "const" "constructor" "continue" "crossinline" "data" "delegate"
  "do" "dynamic" "else" "enum" "expect" "external" "field" "file"
  "final" "finally" "for" "fun" "get" "if" "import" "in"
  "infix" "init" "inline" "inner" "interface" "internal" "is"
  "it" "lateinit" "noinline" "object" "open" "operator" "out"
  "override" "package" "param" "private" "property" "protected"
  "public" "receiver" "reified" "return" "sealed" "set" "setparam"
  "super" "suspend" "tailrec" "this" "throw" "try" "typealias"
  "typeof" "val" "var" "vararg" "when" "where" "while"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_declaration (simple_identifier) @function)
(call_expression (simple_identifier) @function)
(navigation_expression
  (navigation_suffix (simple_identifier) @function))

; ── Types ─────────────────────────────────────────────────────────────────

(class_declaration (type_identifier) @type)
(object_declaration (type_identifier) @type)
(type_identifier) @type

; ── Constants ─────────────────────────────────────────────────────────────

(integer_literal) @constant
(long_literal) @constant
(real_literal) @constant
"true" @constant
"false" @constant
"null" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_literal) @string

; ── Comments ──────────────────────────────────────────────────────────────

(multiline_comment) @comment
(line_comment) @comment
