; Tree-sitter queries for the Janet language provider.

; Top-level function definitions
(defn_form
  name: (symbol) @name) @function

; Shorthand `(defn- ...)` private functions
(defn_form
  name: (symbol) @name) @function

; Variable definitions
(def_form
  name: (symbol) @name) @variable

; var definitions
(var_form
  name: (symbol) @name) @variable
