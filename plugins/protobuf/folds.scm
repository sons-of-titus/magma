; Tree-sitter fold query for Protobuf buffers.
;
; Message/enum/service bodies fold.

(message_body) @fold
(enum_body) @fold
(service_body) @fold
(oneof_body) @fold
