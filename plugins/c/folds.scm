; Tree-sitter fold query for C buffers.
;
; Registered automatically by plugin/define as c-mode's fold query.

(compound_statement) @fold
(field_declaration_list) @fold
(enumerator_list) @fold
(initializer_list) @fold
