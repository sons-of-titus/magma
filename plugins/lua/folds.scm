; Tree-sitter fold query for Lua buffers.
;
; Function bodies and do blocks fold.

(function_body) @fold
(do_statement) @fold
