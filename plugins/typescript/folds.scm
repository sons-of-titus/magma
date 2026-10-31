; Tree-sitter fold query for TypeScript buffers.
;
; Registered automatically by plugin/define as typescript-mode's fold
; query.  Same shapes as JavaScript plus the type-level bodies.

(statement_block) @fold
(class_body) @fold
(object) @fold
(interface_body) @fold
(enum_body) @fold
(object_type) @fold
