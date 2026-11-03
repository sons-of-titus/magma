; Tree-sitter highlight query for cpp buffers.
;
; Registered automatically by plugin/define as cpp-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "auto" "break" "case" "catch" "class" "const" "constexpr"
  "continue" "co_await" "co_return" "co_yield" "default"
  "delete" "do" "else" "enum" "explicit" "export"
  "extern" "final" "for" "friend" "goto" "if" "inline"
  "namespace" "new" "noexcept" "nullptr" "operator" "override"
  "private" "protected" "public" "register" "requires" "return"
  "sizeof" "static" "static_assert" "struct" "switch" "template"
  "this" "throw" "try" "typedef" "typename" "union"
  "unsigned" "signed" "using" "virtual" "volatile" "while"
] @keyword

; ── Types ─────────────────────────────────────────────────────────────────

(type_identifier) @type
(primitive_type) @type
(class_specifier name: (type_identifier) @type)
(struct_specifier name: (type_identifier) @type)
(enum_specifier name: (type_identifier) @type)
(namespace_identifier) @module

; ── Functions ─────────────────────────────────────────────────────────────

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function))
(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @function))
(call_expression function: (identifier) @function)
(call_expression
  function: (field_expression field: (field_identifier) @function))

; ── Preprocessor ──────────────────────────────────────────────────────────

(preproc_function_def name: (identifier) @function)
(preproc_def name: (identifier) @constant)
["#define" "#ifdef" "#ifndef" "#if" "#else" "#elif" "#endif" "#pragma"] @keyword

; ── Properties ────────────────────────────────────────────────────────────

(field_identifier) @property

; ── Constants ─────────────────────────────────────────────────────────────

(number_literal) @constant
"true" @constant
"false" @constant

; ── Strings ───────────────────────────────────────────────────────────────

(string_literal) @string
(system_lib_string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
