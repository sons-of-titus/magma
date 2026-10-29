; Tree-sitter highlight query for kotlin buffers.
;
; Registered automatically by plugin/define as kotlin-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(function_declaration name: (simple_identifier) @function)
(string) @string
(comment) @comment
(number) @constant
"true" @constant
"false" @constant
"null" @constant
