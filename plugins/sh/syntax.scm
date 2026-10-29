; Tree-sitter highlight query for sh buffers.
;
; Registered automatically by plugin/define as sh-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(function_definition name: (word) @function)
(command name: (command_name) @function)
(string) @string
(comment) @comment
(variable_name) @variable
