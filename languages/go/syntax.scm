; Tree-sitter queries for the Go language provider.

(function_declaration
  name: (identifier) @name) @function

(method_declaration
  name: (field_identifier) @name) @method

(type_declaration
  (type_spec
    name: (type_identifier) @name)) @struct

(struct_type) @struct

(interface_type) @interface

(const_declaration
  (const_spec
    name: (identifier) @name)) @constant

(var_declaration
  (var_spec
    name: (identifier) @name)) @variable
