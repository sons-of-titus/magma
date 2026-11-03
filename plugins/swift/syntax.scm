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
  "Any" "Protocol" "Self" "Type" "as" "associatedtype"
  "associativity" "async" "await" "break" "case" "catch" "class"
  "continue" "convenience" "default" "defer" "deinit" "didSet"
  "do" "dynamic" "else" "enum" "extension" "fallthrough"
  "fileprivate" "final" "for" "func" "get" "guard"
  "if" "import" "in" "indirect" "infix" "init" "inout"
  "internal" "is" "lazy" "left" "let" "mutating" "none"
  "nonmutating" "open" "operator" "optional" "override" "postfix"
  "precedencegroup" "prefix" "private" "protocol" "public"
  "rethrows" "return" "right" "set" "some" "static" "struct"
  "subscript" "super" "switch" "throw" "throws" "try" "typealias"
  "unowned" "var" "weak" "where" "while" "willSet"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_declaration name: (simple_identifier) @function)
(call_expression (simple_identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(class_declaration name: (type_identifier) @type)
(struct_declaration name: (type_identifier) @type)
(enum_declaration name: (type_identifier) @type)
(protocol_declaration name: (type_identifier) @type)
(type_identifier) @type

; ── Constants ─────────────────────────────────────────────────────────────

(integer_literal) @constant
(real_literal) @constant
"true" @constant
"false" @constant
"nil" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_literal) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
(multiline_comment) @comment
