; Tree-sitter queries for the Python language provider.

; Function definitions
(function_definition
  name: (identifier) @name) @function

; Async function definitions
(decorated_definition
  definition: (function_definition
    name: (identifier) @name)) @function

; Class definitions
(class_definition
  name: (identifier) @name) @class

; Module-level variable assignments
(assignment
  left: (identifier) @name) @variable
