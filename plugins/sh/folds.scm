; Tree-sitter fold query for shell buffers.
;
; Compound statements fold: if/for/while/case bodies and functions.

(compound_statement) @fold
(case_statement) @fold
(function_definition) @fold
