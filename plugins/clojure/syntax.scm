; Tree-sitter highlight query for clojure buffers.
;
; Registered automatically by plugin/define as clojure-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (sym) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(sym_lit) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

(kwd_lit) @keyword

; ── Constants ─────────────────────────────────────────────────────────────

(num_lit) @constant
(bool_lit) @constant
"nil" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(str_lit) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
(dis_expr) @comment
