; Tree-sitter highlight query for lua buffers.
;
; Registered automatically by plugin/define as lua-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(function_declaration name: (identifier) @function)
(function_call name: (identifier) @function)

(string) @string
(comment) @comment
(number) @constant
"true" @constant
"false" @constant
"nil" @constant
