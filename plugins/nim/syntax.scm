; Tree-sitter highlight query for nim buffers.
;
; Registered automatically by plugin/define as nim-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(proc_declaration name: (identifier) @function)
(func_declaration name: (identifier) @function)
(type_declaration name: (identifier) @type)
(string) @string
(comment) @comment
(number) @constant
