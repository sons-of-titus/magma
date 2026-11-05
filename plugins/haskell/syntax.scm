; Tree-sitter highlight query for haskell buffers.
;
; Registered automatically by plugin/define as haskell-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (variable) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(variable) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "as" "case" "class" "data" "default" "deriving" "do"
  "else" "foreign" "hiding" "if" "import" "in" "infix"
  "infixl" "infixr" "instance" "let" "module" "newtype"
  "of" "qualified" "then" "type" "where"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function (variable) @function)
(signature (variable) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(type) @type
(constructor) @type

; ── Constants ─────────────────────────────────────────────────────────────

(integer) @constant
(float) @constant
((constructor) @constant
 (#eq? @constant "True"))
((constructor) @constant
 (#eq? @constant "False"))

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(char) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
