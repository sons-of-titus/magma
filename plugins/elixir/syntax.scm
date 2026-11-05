; Tree-sitter highlight query for elixir buffers.
;
; Registered automatically by plugin/define as elixir-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "after" "and" "catch" "do" "else" "end" "fn"
  "in" "not" "or" "rescue" "when"
] @keyword
((identifier) @keyword
 (#any-of? @keyword "for" "if" "try" "unless" "with"))

; ── Functions / macros ────────────────────────────────────────────────────

(call target: (identifier) @function)
(call target: (dot . (alias) @module . (identifier) @function))

; ── Types / modules ───────────────────────────────────────────────────────

(alias) @type

; ── Atoms ─────────────────────────────────────────────────────────────────

(atom) @constant
(quoted_atom) @constant

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant
"true" @constant
"false" @constant
"nil" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
