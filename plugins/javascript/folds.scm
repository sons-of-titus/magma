; Tree-sitter fold query for JavaScript buffers.
;
; Registered automatically by plugin/define as javascript-mode's fold
; query.  Statement blocks cover function/method bodies and control
; flow; class bodies and object literals fold on their braces.

(statement_block) @fold
(class_body) @fold
(object) @fold
