; Tree-sitter fold query for Swift buffers.
;
; Function bodies, type bodies, and closures fold.

(function_body) @fold
(class_body) @fold
(protocol_body) @fold
(enum_type_body) @fold
(struct_type_body) @fold
(closure_expr) @fold
