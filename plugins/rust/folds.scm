; Tree-sitter fold query for Rust buffers.
;
; Registered automatically by plugin/define as rust-mode's fold query;
; each @fold capture is a foldable range the gutter offers to collapse.
; Bodies are blocks, so (block) covers functions, control flow, match
; arms, and closures; the list nodes cover impl/trait/mod, struct/union,
; and enum bodies.

(block) @fold
(declaration_list) @fold
(field_declaration_list) @fold
(enum_variant_list) @fold
