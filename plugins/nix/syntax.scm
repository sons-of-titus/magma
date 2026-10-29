; Tree-sitter highlight query for nix buffers.
;
; Registered automatically by plugin/define as nix-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable
(string) @string
(comment) @comment
(integer) @constant
"true" @constant
"false" @constant
"null" @constant
