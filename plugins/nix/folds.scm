; Tree-sitter fold query for Nix buffers.
;
; Attribute sets and let bindings fold.

(attrset) @fold
(let_expression) @fold
(rec_attrset_expression) @fold
