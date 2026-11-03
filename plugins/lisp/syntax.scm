; Tree-sitter highlight query for lisp buffers.
;
; Registered automatically by plugin/define as lisp-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (symbol) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(symbol) @variable

; ── Functions ─────────────────────────────────────────────────────────────

(defun name: (symbol) @function)
(defmacro name: (symbol) @function)
(list . (symbol) @function)

; ── Constants ─────────────────────────────────────────────────────────────

(number) @constant
"t" @constant
"nil" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
(block_comment) @comment
