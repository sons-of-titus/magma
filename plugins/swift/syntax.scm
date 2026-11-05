; Tree-sitter highlight query for swift buffers.
;
; Registered automatically by plugin/define as swift-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "Protocol" "Type" "as" "associatedtype"
  "async" "await" "break" "case" "class"
  "continue" "convenience" "deinit" "didSet"
  "do" "dynamic" "enum" "extension" "fallthrough"
  "fileprivate" "final" "for" "func" "get" "guard"
  "if" "import" "in" "indirect" "infix" "init" "inout"
  "internal" "is" "lazy" "let" "mutating"
  "nonmutating" "open" "operator" "optional" "override" "postfix"
  "precedencegroup" "prefix" "private" "protocol" "public"
  "return" "set" "some" "static" "struct"
  "subscript" "switch" "try" "typealias"
  "unowned" "var" "weak" "while" "willSet"
] @keyword
(catch_keyword) @keyword
(default_keyword) @keyword
(else) @keyword
(throw_keyword) @keyword
(throws) @keyword
(where_keyword) @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_declaration name: (simple_identifier) @function)
(call_expression (simple_identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(class_declaration name: (type_identifier) @type)
(protocol_declaration name: (type_identifier) @type)
(type_identifier) @type

; ── Constants ─────────────────────────────────────────────────────────────

(integer_literal) @constant
(real_literal) @constant
"true" @constant
"false" @constant
(nil_literal) @constant

; ── Strings ───────────────────────────────────────────────────────────────

[
  (line_string_literal)
  (multi_line_string_literal)
  (raw_string_literal)
] @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
(multiline_comment) @comment
