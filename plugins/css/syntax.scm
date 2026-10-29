; Tree-sitter highlight query for css buffers.
;
; Registered automatically by plugin/define as css-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(tag_name) @type
(class_selector (class_name) @function)
(id_selector (id_name) @function)
(property_name) @property
(string_value) @string
(integer_value) @constant
(float_value) @constant
(comment) @comment
