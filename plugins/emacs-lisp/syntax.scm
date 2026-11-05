; Tree-sitter highlight query for emacs-lisp buffers.
;
; Registered automatically by plugin/define as emacs-lisp-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (symbol) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(symbol) @variable

; ── Functions / macros ────────────────────────────────────────────────────

(function_definition name: (symbol) @function)
(macro_definition name: (symbol) @function)

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant
((symbol) @constant (#eq? @constant "t"))
((symbol) @constant (#eq? @constant "nil"))

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
