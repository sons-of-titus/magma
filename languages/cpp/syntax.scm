; Tree-sitter queries for the C++ language provider.

(function_definition
  name: (identifier) @name) @function

(declaration
  type: (class_specifier
    name: (type_identifier) @name)) @class

(declaration
  type: (struct_specifier
    name: (type_identifier) @name)) @struct

(declaration
  type: (enum_specifier
    name: (type_identifier) @name)) @enum

(template_declaration
  (function_definition
    name: (identifier) @name)) @function

(template_declaration
  (declaration
    type: (class_specifier
      name: (type_identifier) @name))) @class

(field_declaration
  (identifier) @name) @variable
