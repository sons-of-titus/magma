; Tree-sitter highlight query for Rust buffers.
;
; Registered automatically by plugin/define as rust-mode's highlight query;
; capture names resolve to faces through the scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first: every specific
; pattern below overrides it on the tokens it names (call targets, macro
; names, function definitions).

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "as" "async" "await" "break" "const" "continue"
  "dyn" "else" "enum" "extern" "fn" "for" "if" "impl" "in" "let" "loop"
  "match" "mod" "move" "pub" "ref" "return" "static" "struct"
  "trait" "type" "union" "unsafe" "use" "where" "while"
  (mutable_specifier)
] @keyword

; ── Fields (generic — must precede the call rules below so a method
;    name in a call position paints as function over property) ─────────────

(field_declaration name: (field_identifier) @property)
(field_expression field: (field_identifier) @property)
(field_initializer field: (field_identifier) @property)
(shorthand_field_identifier) @variable
(shorthand_field_initializer (identifier) @property)
(self) @variable

; ── Paths ─────────────────────────────────────────────────────────────────
;
; A path reads as a path: every segment before the name it heads is a module
; (`std::time::Instant`, `crate::render::frame`).  Case predicates would
; separate uppercase type segments from lowercase ones, but the runtime does
; not apply query predicates, so the split cannot be made here.

(scoped_identifier path: (identifier) @module)
(scoped_identifier (scoped_identifier name: (identifier) @module))
(scoped_type_identifier path: (identifier) @type)
(scoped_type_identifier (scoped_identifier name: (identifier) @type))
(use_declaration argument: (identifier) @module)
(use_declaration argument: (scoped_identifier name: (identifier) @module))
(scoped_use_list path: (identifier) @module)
(scoped_use_list path: (scoped_identifier name: (identifier) @module))
(use_list (identifier) @module)
(use_list (self) @module)

; ── Functions and macros ──────────────────────────────────────────────────

(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression
  function: (scoped_identifier name: (identifier) @function))
(call_expression
  function: (field_expression field: (field_identifier) @function))
(generic_function function: (identifier) @function)
(generic_function
  function: (scoped_identifier name: (identifier) @function))
(generic_function
  function: (field_expression field: (field_identifier) @function))
(macro_invocation macro: (identifier) @function)
(macro_invocation macro: (scoped_identifier name: (identifier) @function))
(macro_definition name: (identifier) @function)

; ── Types ─────────────────────────────────────────────────────────────────

(type_identifier) @type
(primitive_type) @type

; Declarations paint their name with the type scope.
(struct_item name: (type_identifier) @type)
(enum_item name: (type_identifier) @type)
(union_item name: (type_identifier) @type)
(trait_item name: (type_identifier) @type)
(type_item name: (type_identifier) @type)
(impl_item trait: (type_identifier) @type)
(mod_item name: (identifier) @module)

; ── Constants ─────────────────────────────────────────────────────────────

(const_item name: (identifier) @constant)
(enum_variant name: (identifier) @constant)
(lifetime) @constant
(escape_sequence) @constant
(boolean_literal) @constant
(integer_literal) @constant
(float_literal) @constant
(char_literal) @constant

; Common operator spellings.
["+" "-" "*" "/" "%" "=" "==" "!=" "<" ">" "<=" ">=" "&&" "||" "!"
 "&" "|" "^" "<<" ">>" "+=" "-=" "*=" "/=" "%="] @operator

["(" ")" "[" "]" "{" "}"] @punctuation
["," "." ":" "::" ";" "?" "->" "=>"] @punctuation

; ── Strings, comments, attributes ─────────────────────────────────────────

(string_literal) @string
(raw_string_literal) @string
(attribute_item) @keyword
(inner_attribute_item) @keyword

; Comments come after operators and punctuation so markers like `///`, `//!`,
; `/**`, `/*!` and any punctuation inside comments stay comments instead of
; being painted as operators or punctuation.
[
  (line_comment)
  (block_comment)
  (outer_doc_comment_marker)
  (inner_doc_comment_marker)
  (doc_comment)
] @comment

(ERROR) @error
