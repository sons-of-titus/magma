; Tree-sitter highlight query for lisp buffers.
;
; Registered automatically by plugin/define as lisp-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (symbol) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(sym_lit) @variable

; ── Functions ─────────────────────────────────────────────────────────────

(defun_header function_name: (sym_lit) @function)
(list_lit . (sym_lit) @function)

; ── Constants ─────────────────────────────────────────────────────────────

(num_lit) @constant
(nil_lit) @constant
((sym_lit) @constant (#eq? @constant "t"))
(kwd_lit) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(str_lit) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
(block_comment) @comment
