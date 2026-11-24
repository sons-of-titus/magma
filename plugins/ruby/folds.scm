; Tree-sitter fold query for Ruby buffers.
;
; Method/class/module bodies are body statements; do blocks and case
; bodies fold as well.

(body_statement) @fold
(do_block) @fold
(case) @fold
(begin_block) @fold
