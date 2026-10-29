; Tree-sitter highlight query for go buffers.
;
; Registered automatically by plugin/define as go-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(function_declaration name: (identifier) @function)
(method_declaration name: (field_identifier) @function)
(type_spec name: (type_identifier) @type)
(call_expression function: (identifier) @function)
(field_identifier) @property

(interpreted_string_literal) @string
(raw_string_literal) @string
(comment) @comment
(int_literal) @constant
(float_literal) @constant
"true" @constant
"false" @constant
"nil" @constant
