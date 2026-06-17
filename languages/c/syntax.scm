; Tree-sitter queries for the C language provider.

(function_definition
  name: (identifier) @name) @function

(declaration
  type: (struct_specifier
    name: (type_identifier) @name)) @struct

(declaration
  type: (union_specifier
    name: (type_identifier) @name)) @struct

(declaration
  type: (enum_specifier
    name: (type_identifier) @name)) @enum

(preproc_function_def
  name: (identifier) @name) @function

(declaration
  (init_declarator
    (identifier) @name)) @variable
