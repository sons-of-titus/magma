; Tree-sitter highlight query for toml buffers.
;
; Registered automatically by plugin/define as toml-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(table_name) @module
(string) @string
(comment) @comment
(number) @constant
"true" @constant
"false" @constant
