; Tree-sitter highlight query for typescript buffers.
;
; Registered automatically by plugin/define as typescript-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.
; Captures land in query order and the later span paints over the earlier
; one, so the generic (identifier) fallback comes first.

; ── Fallback ──────────────────────────────────────────────────────────────

(identifier) @variable

; ── Keywords ──────────────────────────────────────────────────────────────

[
  "abstract" "as" "async" "await" "break" "case" "catch" "class"
  "const" "continue" "debugger" "declare" "default" "delete"
  "do" "else" "enum" "export" "extends" "finally" "for" "from"
  "function" "if" "implements" "import" "in" "instanceof"
  "interface" "let" "module" "namespace" "new" "of" "override"
  "private" "protected" "public" "readonly" "return" "static"
  "super" "switch" "throw" "try" "type" "typeof" "var"
  "void" "while" "with" "yield"
] @keyword

; ── Functions ─────────────────────────────────────────────────────────────

(function_declaration name: (identifier) @function)
(function name: (identifier) @function)
(method_definition name: (property_identifier) @function)
(call_expression function: (identifier) @function)
(call_expression
  function: (member_expression property: (property_identifier) @function))

; ── Types ─────────────────────────────────────────────────────────────────

(class_declaration name: (type_identifier) @type)
(class name: (type_identifier) @type)
(interface_declaration name: (type_identifier) @type)
(type_alias_declaration name: (type_identifier) @type)
(enum_declaration name: (identifier) @type)
(type_identifier) @type

; ── Properties ────────────────────────────────────────────────────────────

(member_expression property: (property_identifier) @property)

; ── Constants ─────────────────────────────────────────────────────────────

(number) @constant
"true" @constant
"false" @constant
"null" @constant
"undefined" @constant
(this) @variable

; ── Strings ───────────────────────────────────────────────────────────────

(string) @string
(template_string) @string

; ── Comments ──────────────────────────────────────────────────────────────

(comment) @comment
