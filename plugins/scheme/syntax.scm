; Tree-sitter highlight query for scheme buffers.
;
; Registered automatically by plugin/define as scheme-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Functions / definitions ───────────────────────────────────────────────

(list . (identifier) @function)

; ── Constants ─────────────────────────────────────────────────────────────

(number) @constant
"#t" @constant
"#f" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
(block_comment) @comment
