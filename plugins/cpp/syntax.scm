; Tree-sitter highlight query for cpp buffers.
;
; Registered automatically by plugin/define as cpp-mode's
; highlight query; capture names resolve to faces through the
; scope-face table.

(identifier) @variable

(type_identifier) @type
(primitive_type) @type

(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function))
(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @function))
(call_expression
  function: (identifier) @function)
(class_specifier
  name: (type_identifier) @type)
(field_identifier) @property

(preproc_function_def
  name: (identifier) @function)
(preproc_def
  name: (identifier) @constant)

(string_literal) @string
(number_literal) @constant
(comment) @comment
