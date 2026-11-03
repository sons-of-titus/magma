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

(defun name: (symbol) @function)
(defmacro name: (symbol) @function)
(function_call function: (symbol) @function)

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant
"t" @constant
"nil" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
