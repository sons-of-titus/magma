; Tree-sitter queries for the Ruby language provider.

(method
  name: (identifier) @name) @function

(singleton_method
  name: (identifier) @name) @function

(class
  name: (constant) @name) @class

(module
  name: (constant) @name) @module

(assignment
  left: (identifier) @name) @variable

(call
  method: (identifier) @name) @function
