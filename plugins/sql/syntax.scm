; Tree-sitter highlight query for sql buffers.
;
; Registered automatically by plugin/define as sql-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable
(string) @string
(number) @constant
(comment) @comment
