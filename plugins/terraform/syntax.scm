; Tree-sitter highlight query for terraform buffers.
;
; Registered automatically by plugin/define as terraform-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(string) @string
(comment) @comment
(number) @constant
"true" @constant
"false" @constant
"null" @constant
