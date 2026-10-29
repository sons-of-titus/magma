; Tree-sitter highlight query for lisp buffers.
;
; Registered automatically by plugin/define as lisp-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(defun name: (symbol) @function)
(defmacro name: (symbol) @function)
(string) @string
(comment) @comment
(number) @constant
