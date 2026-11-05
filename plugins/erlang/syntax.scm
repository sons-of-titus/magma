; Tree-sitter highlight query for erlang buffers.
;
; Registered automatically by plugin/define as erlang-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (atom) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(atom) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "after" "and" "andalso" "band" "begin" "bnot" "bor" "bsl"
  "bsr" "bxor" "case" "catch" "div" "end" "fun"
  "if" "maybe" "not" "of" "or" "orelse" "receive"
  "rem" "try" "when" "xor"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_clause name: (atom) @function)
(call expr: (atom) @function)
(call expr: (remote fun: (atom) @function))
(remote module: (remote_module module: (atom) @module))

; ── Types / records ───────────────────────────────────────────────────────

(type_name (atom) @type)

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(char) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
