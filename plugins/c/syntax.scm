; Tree-sitter highlight query for C buffers.
;
; Registered automatically by plugin/define as c-mode's highlight query;
; capture names resolve to faces through the scope-face table.
; Generic captures come first so more specific ones paint over them.

(identifier) @variable

(type_identifier) @type
(primitive_type) @type

; Functions: definitions, calls, macro definitions.
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function))
(call_expression
  function: (identifier) @function)
(preproc_function_def
  name: (identifier) @function)

; Preprocessor constants (#define NAME without a body).
(preproc_def
  name: (identifier) @constant)

; Enumerators are constants.
(enumerator
  name: (identifier) @constant)

; Includes: <stdio.h> and friends.
(system_lib_string) @string
(string_literal) @string

(char_literal) @constant
(number_literal) @constant

(comment) @comment
