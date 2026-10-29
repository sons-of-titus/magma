# Additional language keyword tables and the extension -> language map.
#
# Tables register through the same native rule registry as
# syntax_langs.janet; define-syntax is defined there and loads first.

(define-syntax "clojure"
  ["defn" "def" "defmacro" "defmethod" "defmulti" "defonce"
   "let" "fn" "if" "when" "when-not" "unless" "case" "cond"
   "do" "for" "each" "map" "filter" "reduce"
   "try" "catch" "finally" "throw"
   "import" "require" "use" "ns" "in-ns"
   "true" "false" "nil"
   "->" "->>" "some->" "as->" "comp" "partial"
   "atom" "ref" "agent" "swap!" "reset!"])

(define-syntax "sql"
  ["SELECT" "FROM" "WHERE" "INSERT" "INTO" "VALUES"
   "UPDATE" "SET" "DELETE" "CREATE" "TABLE" "ALTER" "DROP"
   "INDEX" "VIEW" "JOIN" "LEFT" "RIGHT" "INNER" "OUTER" "ON"
   "GROUP" "BY" "ORDER" "ASC" "DESC" "HAVING" "LIMIT" "OFFSET"
   "DISTINCT" "COUNT" "SUM" "AVG" "MIN" "MAX" "AS"
   "AND" "OR" "NOT" "IN" "LIKE" "BETWEEN" "IS" "NULL"
   "IF" "ELSE" "BEGIN" "END" "CASE" "WHEN" "THEN"])

(define-syntax "toml"
  ["true" "false" "date" "time"])

(define-syntax "yaml"
  ["true" "false" "yes" "no" "on" "off" "null"])

(define-syntax "python"
  ["def" "class" "return" "if" "elif" "else" "for" "while"
   "import" "from" "as" "try" "except" "finally" "with"
   "yield" "lambda" "pass" "break" "continue"
   "True" "False" "None"
   "self" "cls"
   "print" "len" "range" "int" "str" "float" "list" "dict" "set"
   "is" "in" "not" "and" "or"])

(define-syntax "json"
  ["true" "false" "null"])

(define-syntax "emacs-lisp"
  ["defun" "defvar" "defcustom" "defmacro" "defadvice"
   "defgroup" "defface" "defconst"
   "let" "let*" "if" "when" "unless" "cond" "case" "progn"
   "save-excursion" "with-current-buffer" "save-restriction"
   "lambda" "mapc" "mapcar" "dolist" "dotimes"
   "setq" "setq-local" "add-hook" "remove-hook"
   "require" "provide" "eval-when-compile"
   "t" "nil"])

(define-syntax "lisp"
  ["defun" "defvar" "defmacro" "defconstant" "defparameter"
   "let" "let*" "if" "when" "unless" "cond" "case" "progn"
   "lambda" "mapcar" "dolist" "dotimes"
   "setq" "quote" "function"
   "t" "nil"])

(define-syntax "scheme"
  ["define" "let" "let*" "letrec" "if" "cond" "case"
   "when" "unless" "lambda" "begin" "do" "set!"
   "quote" "quasiquote" "unquote" "splice"
   "call-with-current-continuation" "call/cc"
   "map" "filter" "fold" "display" "newline"
   "#t" "#f"])

(define-syntax "fish"
  ["function" "if" "else" "for" "while" "begin" "end"
   "return" "break" "continue"
   "set" "local" "export" "true" "false"
   "switch" "case" "status" "argparse" "read" "echo"
   "command" "not" "and" "or"])

(define-syntax "html"
  ["html" "head" "body" "div" "span" "p" "a" "img"
   "table" "tr" "td" "th" "form" "input" "button"
   "select" "option" "textarea"
   "ul" "ol" "li" "h1" "h2" "h3" "h4" "h5" "h6"
   "script" "style" "link" "meta" "title"
   "section" "article" "header" "footer" "nav" "main"
   "aside" "figure" "figcaption" "pre" "code"
   "em" "strong" "blockquote" "cite"])

(define-syntax "css"
  ["color" "background" "background-color" "margin" "padding"
   "border" "width" "height" "display" "position"
   "top" "left" "right" "bottom"
   "font-size" "font-family" "font-weight"
   "text-align" "text-decoration"
   "overflow" "z-index" "opacity" "content" "important"])

(define-syntax "xml"
  ["xml" "version" "encoding" "schema" "stylesheet"
   "transform" "template" "element" "attribute"
   "value-of" "for-each" "if" "choose" "when" "otherwise"])

(define-syntax "markdown"
  ["true" "false"])

(define-syntax "org"
  ["true" "false"])

(define-syntax "terraform"
  ["resource" "data" "variable" "output" "provider"
   "terraform" "module" "locals"
   "lifecycle" "count" "for_each" "depends_on"
   "provisioner" "connection"])

(define-syntax "protobuf"
  ["syntax" "package" "import" "option"
   "message" "enum" "service" "rpc" "returns"
   "repeated" "required" "optional"
   "extends" "oneof" "map" "reserved" "extend" "stream"])

# --- Extension-to-language map -----------------------------------------

# Populate the table defined in syntax_core.janet — use put, never def,
# so the forward declaration binding in syntax_core's module is mutated.
(syntax/map-extension "janet" "janet")
(syntax/map-extension "jdn" "janet")
(syntax/map-extension "rs" "rust")
(syntax/map-extension "py" "python")
(syntax/map-extension "pyi" "python")
(syntax/map-extension "pyw" "python")
(syntax/map-extension "js" "javascript")
(syntax/map-extension "jsx" "javascript")
(syntax/map-extension "ts" "typescript")
(syntax/map-extension "tsx" "typescript")
(syntax/map-extension "go" "go")
(syntax/map-extension "c" "c")
(syntax/map-extension "h" "c")
(syntax/map-extension "cpp" "cpp")
(syntax/map-extension "cc" "cpp")
(syntax/map-extension "cxx" "cpp")
(syntax/map-extension "hpp" "cpp")
(syntax/map-extension "rb" "ruby")
(syntax/map-extension "sh" "sh")
(syntax/map-extension "bash" "sh")
(syntax/map-extension "zsh" "sh")
(syntax/map-extension "lua" "lua")
(syntax/map-extension "java" "java")
(syntax/map-extension "kt" "kotlin")
(syntax/map-extension "swift" "swift")
(syntax/map-extension "hs" "haskell")
(syntax/map-extension "ex" "elixir")
(syntax/map-extension "exs" "elixir")
(syntax/map-extension "erl" "erlang")
(syntax/map-extension "nim" "nim")
(syntax/map-extension "zig" "zig")
(syntax/map-extension "nix" "nix")
(syntax/map-extension "clj" "clojure")
(syntax/map-extension "el" "emacs-lisp")
(syntax/map-extension "lisp" "lisp")
(syntax/map-extension "scm" "scheme")
(syntax/map-extension "fish" "fish")
(syntax/map-extension "sql" "sql")
(syntax/map-extension "toml" "toml")
(syntax/map-extension "yaml" "yaml")
(syntax/map-extension "yml" "yaml")
(syntax/map-extension "json" "json")
(syntax/map-extension "html" "html")
(syntax/map-extension "htm" "html")
(syntax/map-extension "css" "css")
(syntax/map-extension "scss" "css")
(syntax/map-extension "xml" "xml")
(syntax/map-extension "md" "markdown")
(syntax/map-extension "markdown" "markdown")
(syntax/map-extension "org" "org")
(syntax/map-extension "tf" "terraform")
(syntax/map-extension "proto" "protobuf")

(print "syntax language extensions loaded")
