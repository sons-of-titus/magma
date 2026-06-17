; Tree-sitter queries for the TypeScript language provider.

(function_declaration
  name: (identifier) @name) @function

(method_definition
  name: (property_identifier) @name) @method

(class_declaration
  name: (type_identifier) @name) @class

(interface_declaration
  name: (type_identifier) @name) @interface

(type_alias_declaration
  name: (type_identifier) @name) @struct

(enum_declaration
  name: (identifier) @name) @enum

(variable_declarator
  name: (identifier) @name) @variable
