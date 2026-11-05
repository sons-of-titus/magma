; Tree-sitter highlight query for janet buffers.
;
; Registered automatically by plugin/define as janet-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (symbol) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(sym_lit) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

(kwd_lit) @keyword

; ── Functions ─────────────────────────────────────────────────────────────

; defn and def forms: the binding name is the second child of the tuple.
(par_tup_lit
  .
  (sym_lit) @keyword
  (sym_lit) @function
  (#match? @keyword "^(defn|defn-|defmacro|defmacro-|def|var|fn)$"))

; ── Constants ─────────────────────────────────────────────────────────────

(num_lit) @constant
(bool_lit) @constant
(nil_lit) @constant

; ── Strings ───────────────────────────────────────────────────────────────

(str_lit) @string
(long_str_lit) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
