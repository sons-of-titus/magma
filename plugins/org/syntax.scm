; Tree-sitter highlight query for org buffers.
;
; Registered automatically by plugin/define as org-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(headline (stars) @function)
(directive) @keyword
(value) @string
(comment) @comment
