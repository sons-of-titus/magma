; Tree-sitter highlight query for java buffers.
;
; Registered automatically by plugin/define as java-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "abstract" "assert" "break" "case" "catch" "class" "continue"
  "default" "do" "else" "enum" "extends" "final" "finally" "for"
  "if" "implements" "import" "instanceof" "interface" "native" "new"
  "package" "private" "protected" "public" "record" "return"
  "static" "strictfp" "super" "switch" "synchronized" "this"
  "throw" "throws" "transient" "try" "volatile" "while"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(method_declaration name: (identifier) @function)
(method_invocation name: (identifier) @function)
(constructor_declaration name: (identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(class_declaration name: (identifier) @type)
(interface_declaration name: (identifier) @type)
(enum_declaration name: (identifier) @type)
(type_identifier) @type

; ── Constants ─────────────────────────────────────────────────────────────

(decimal_integer_literal) @constant
(hex_integer_literal) @constant
(octal_integer_literal) @constant
(decimal_floating_point_literal) @constant
"true" @constant
"false" @constant
"null" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_literal) @string
(character_literal) @string

; ── Comments ──────────────────────────────────────────────────────────────

[
  (line_comment)
  (block_comment)
] @comment
