; Tree-sitter highlight query for ruby buffers.
;
; Registered automatically by plugin/define as ruby-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(method name: (identifier) @function)
(singleton_method name: (identifier) @function)
(call method: (identifier) @function)
(class name: (constant) @type)
(module name: (constant) @type)

(string) @string
(comment) @comment
(integer) @constant
(float) @constant
"true" @constant
"false" @constant
"nil" @constant
