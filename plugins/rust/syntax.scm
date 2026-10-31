; Tree-sitter highlight query for Rust buffers.
;
; Registered automatically by plugin/define as rust-mode's highlight query;
; capture names resolve to faces through the scope-face table.
; The generic (identifier) capture is last: where a specific pattern and
; the fallback both match (macro names, call targets), the later span
; paints over the earlier one.

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "as" "async" "await" "break" "const" "continue"
  "dyn" "else" "enum" "extern" "fn" "for" "if" "impl" "in" "let" "loop"
  "match" "mod" "move" "pub" "ref" "return" "static" "struct"
  "trait" "type" "union" "unsafe" "use" "where" "while"
  (mutable_specifier)
] @keyword

; ── Fields (generic — must precede the call rules below so a method
;    name in a call position paints as function over property) ─────────────

(field_declaration name: (field_identifier) @property)
(field_expression field: (field_identifier) @property)
(shorthand_field_identifier) @variable
(self) @variable

; ── Functions and macros ──────────────────────────────────────────────────

(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression
  function: (field_expression field: (field_identifier) @function))
(macro_invocation macro: (identifier) @function)
(macro_definition name: (identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(type_identifier) @type
(primitive_type) @type

; Declarations paint their name with the type scope.
(struct_item name: (type_identifier) @type)
(enum_item name: (type_identifier) @type)
(union_item name: (type_identifier) @type)
(trait_item name: (type_identifier) @type)
(type_item name: (type_identifier) @type)
(impl_item trait: (type_identifier) @type)
(mod_item name: (identifier) @module)

; ── Constants ─────────────────────────────────────────────────────────────

(const_item name: (identifier) @constant)
(lifetime) @constant
(escape_sequence) @constant
(boolean_literal) @constant
(integer_literal) @constant
(float_literal) @constant
(char_literal) @constant

; ── Strings, comments, attributes ─────────────────────────────────────────

(string_literal) @string
(raw_string_literal) @string
(line_comment) @comment
(block_comment) @comment
(attribute_item) @keyword
(inner_attribute_item) @keyword

; Common operator spellings.
["+" "-" "*" "/" "%" "=" "==" "!=" "<" ">" "<=" ">=" "&&" "||" "!"
 "&" "|" "^" "<<" ">>" "+=" "-=" "*=" "/=" "%="] @operator

["(" ")" "[" "]" "{" "}"] @punctuation
["," "." ":" "::" ";" "?" "->" "=>"] @punctuation

; Generic fallback: plain identifiers.
(identifier) @variable

(ERROR) @error
