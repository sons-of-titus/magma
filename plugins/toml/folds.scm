; Tree-sitter fold query for TOML buffers.
;
; Tables fold their key/value runs.

(table) @fold
(table_array) @fold
