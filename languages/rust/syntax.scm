; Tree-sitter queries for the Rust language provider.
;
; These queries are read by the TreesitterLanguageProvider to extract
; symbols and navigation targets from Rust source files.

; Function definitions
(function_item
  name: (identifier) @name) @function

; Method definitions inside impl blocks
(impl_item
  (function_item
    name: (identifier) @name)) @method

; Struct definitions
(struct_item
  name: (type_identifier) @name) @struct

; Enum definitions
(enum_item
  name: (type_identifier) @name) @enum

; Trait definitions
(trait_item
  name: (type_identifier) @name) @interface

; Module declarations
(mod_item
  name: (identifier) @name) @module

; Constant declarations
(const_item
  name: (identifier) @name) @constant

; Static items
(static_item
  name: (identifier) @name) @constant

; Type aliases
(type_item
  name: (type_identifier) @name) @struct
