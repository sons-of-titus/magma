; Tree-sitter fold query for Go buffers.
;
; Registered automatically by plugin/define as go-mode's fold query.
; Blocks cover function bodies and control flow; field declaration
; lists cover struct bodies; interface types fold their method sets.

(block) @fold
(field_declaration_list) @fold
(interface_type) @fold
