; Tree-sitter queries for the JavaScript language provider.

(function_declaration
  name: (identifier) @name) @function

(arrow_function) @function

(method_definition
  name: (property_identifier) @name) @method

(class_declaration
  name: (identifier) @name) @class

(variable_declarator
  name: (identifier) @name) @variable

(export_statement
  (function_declaration
    name: (identifier) @name)) @function
