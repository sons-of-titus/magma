; Tree-sitter fold query for Janet buffers.
;
; Registered automatically by plugin/define as janet-mode's fold query.
; The language is parenthesized data: fold the long forms — definitions
; and module-level calls — rather than every list.

(list) @fold
(tuple) @fold
