; Tree-sitter highlight query for yaml buffers.
;
; Registered automatically by plugin/define as yaml-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(comment) @comment
(string_scalar) @string
(double_quote_scalar) @string
(single_quote_scalar) @string
(integer_scalar) @constant
(float_scalar) @constant
(boolean_scalar) @constant
(null_scalar) @constant
