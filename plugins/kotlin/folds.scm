; Tree-sitter fold query for Kotlin buffers.
;
; Function bodies, class bodies, enum bodies, and lambdas fold.

(function_body) @fold
(class_body) @fold
(enum_class_body) @fold
(anonymous_function) @fold
(lambda_literal) @fold
(when_body) @fold
