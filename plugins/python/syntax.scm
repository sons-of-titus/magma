; Tree-sitter highlight query for python buffers.
;
; Registered automatically by plugin/define as python-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(function_definition name: (identifier) @function)
(class_definition name: (identifier) @type)
(call function: (identifier) @function)

(string) @string
(comment) @comment
(integer) @constant
(float) @constant
"True" @constant
"False" @constant
"None" @constant
