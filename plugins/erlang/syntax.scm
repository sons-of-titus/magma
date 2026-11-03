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
  "bsr" "bxor" "case" "catch" "cond" "div" "end" "fun"
  "if" "let" "maybe" "not" "of" "or" "orelse" "receive"
  "rem" "try" "when" "xor"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function name: (atom) @function)
(call target: (remote_call module: (atom) @module function: (atom) @function))
(call target: (local_call function: (atom) @function))

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
