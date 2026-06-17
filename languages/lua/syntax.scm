; Tree-sitter queries for the Lua language provider.

(function_declaration
  name: (identifier) @name) @function

(function_definition
  name: (identifier) @name) @function

(local_function_declaration
  name: (identifier) @name) @function

(assignment_statement
  (variable_list
    name: (identifier) @name)) @variable

(table_constructor) @struct
