; Tree-sitter fold query for Java buffers.
;
; Registered automatically by plugin/define as java-mode's fold query.
; Method and constructor bodies are blocks; class/enum/interface/record
; bodies are their own list nodes.

(block) @fold
(class_body) @fold
(enum_body) @fold
(interface_body) @fold
(array_initializer) @fold
