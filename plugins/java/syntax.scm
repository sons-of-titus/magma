; Tree-sitter highlight query for java buffers.
;
; Registered automatically by plugin/define as java-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(method_declaration name: (identifier) @function)
(class_declaration name: (identifier) @type)
(interface_declaration name: (identifier) @type)
(method_invocation name: (identifier) @function)

(string_literal) @string
(line_comment) @comment
(block_comment) @comment
(decimal_integer_literal) @constant
(decimal_floating_point_literal) @constant
"true" @constant
"false" @constant
"null" @constant
