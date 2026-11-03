; Tree-sitter highlight query for protobuf buffers.
;
; Registered automatically by plugin/define as protobuf-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "enum" "extend" "extensions" "import" "map" "message"
  "oneof" "option" "optional" "package" "repeated"
  "required" "reserved" "returns" "rpc" "service" "stream"
  "syntax" "to" "weak"
] @keyword

; ── Types ─────────────────────────────────────────────────────────────────

(message_name) @type
(enum_name) @type
(service_name) @type
(message_or_enum_type) @type

; ── Fields / RPC names ────────────────────────────────────────────────────

(field (field_name) @property)
(rpc_name) @function

; ── Constants ─────────────────────────────────────────────────────────────

(int_lit) @constant
(float_lit) @constant
(bool_lit) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_literal) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
