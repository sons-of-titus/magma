; Tree-sitter highlight query for zig buffers.
;
; Registered automatically by plugin/define as zig-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)
(struct_declaration name: (identifier) @type)
(enum_declaration name: (identifier) @type)
(string) @string
(line_comment) @comment
(number) @constant
