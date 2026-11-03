; Tree-sitter highlight query for yaml buffers.
;
; Registered automatically by plugin/define as yaml-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

; ── Structure / keys ──────────────────────────────────────────────────────

(block_mapping_pair key: (flow_node) @property)
(block_mapping_pair key: (block_node) @property)
(flow_mapping (flow_pair key: (flow_node) @property))

; ── Constants ─────────────────────────────────────────────────────────────

(integer_scalar) @constant
(float_scalar) @constant
(boolean_scalar) @constant
(null_scalar) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_scalar) @string
(double_quote_scalar) @string
(single_quote_scalar) @string
(block_scalar) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
