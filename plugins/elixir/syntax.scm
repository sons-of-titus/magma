; Tree-sitter highlight query for elixir buffers.
;
; Registered automatically by plugin/define as elixir-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(call target: (identifier) @function)
(string) @string
(comment) @comment
(number) @constant
"true" @constant
"false" @constant
"nil" @constant
