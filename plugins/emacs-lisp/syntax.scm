; Tree-sitter highlight query for emacs-lisp buffers.
;
; Registered automatically by plugin/define as emacs-lisp-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(defun name: (symbol) @function)
(function name: (symbol) @function)
(string) @string
(comment) @comment
(integer) @constant
