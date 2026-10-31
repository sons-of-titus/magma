; Tree-sitter fold query for C++ buffers.
;
; Registered automatically by plugin/define as cpp-mode's fold query.
; The declaration list covers class and namespace bodies.

(compound_statement) @fold
(declaration_list) @fold
(field_declaration_list) @fold
(enumerator_list) @fold
(initializer_list) @fold
