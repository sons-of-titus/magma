; Tree-sitter highlight query for swift buffers.
;
; Registered automatically by plugin/define as swift-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(function_declaration name: (identifier) @function)
(class_declaration name: (identifier) @type)
(struct_declaration name: (identifier) @type)
(call_expression function: (identifier) @function)

(string_literal) @string
(comment) @comment
(integer_literal) @constant
(float_literal) @constant
"true" @constant
"false" @constant
"nil" @constant
