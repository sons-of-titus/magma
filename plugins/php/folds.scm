; Tree-sitter fold query for PHP buffers.
;
; Class/interface/trait declaration lists and function bodies fold.

(declaration_list) @fold
(function_definition) @fold
(method_declaration) @fold
