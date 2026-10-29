; Tree-sitter highlight query for Rust buffers.
;
; Registered automatically by plugin/define as rust-mode's highlight query;
; capture names resolve to faces through the scope-face table.
; Generic captures come first so more specific ones paint over them.

(identifier) @variable

(type_identifier) @type
(primitive_type) @type

; Functions: definitions, calls, macros.
(function_item
  name: (identifier) @function)
(call_expression
  function: (identifier) @function)
(macro_invocation
  macro: (identifier) @function)

; Struct and impl fields.
(field_declaration
  name: (field_identifier) @property)
(field_expression
  field: (field_identifier) @property)

(self) @variable
(mutable_specifier) @keyword
(lifetime) @constant

; Literals.
(string_literal) @string
(raw_string_literal) @string
(char_literal) @constant
(integer_literal) @constant
(float_literal) @constant
(boolean_literal) @constant
(escape_sequence) @constant

; Comments, including /// doc comments.
(line_comment) @comment
(block_comment) @comment
