; Tree-sitter highlight query for javascript buffers.
;
; Registered automatically by plugin/define as javascript-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(function_declaration name: (identifier) @function)
(arrow_function) @function
(method_definition name: (property_identifier) @function)
(class_declaration name: (identifier) @type)
(call_expression function: (identifier) @function)

(string) @string
(template_string) @string
(comment) @comment
(number) @constant
"true" @constant
"false" @constant
"null" @constant
"undefined" @constant
(this) @variable
