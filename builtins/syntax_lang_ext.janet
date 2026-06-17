# Additional language keyword tables and maps.

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
(put *extension-language* "janet" "janet")
(put *extension-language* "jdn" "janet")
(put *extension-language* "rs" "rust")
(put *extension-language* "py" "python")
(put *extension-language* "pyi" "python")
(put *extension-language* "pyw" "python")
(put *extension-language* "js" "javascript")
(put *extension-language* "jsx" "javascript")
(put *extension-language* "ts" "typescript")
(put *extension-language* "tsx" "typescript")
(put *extension-language* "go" "go")
(put *extension-language* "c" "c")
(put *extension-language* "h" "c")
(put *extension-language* "cpp" "cpp")
(put *extension-language* "cc" "cpp")
(put *extension-language* "cxx" "cpp")
(put *extension-language* "hpp" "cpp")
(put *extension-language* "rb" "ruby")
(put *extension-language* "sh" "sh")
(put *extension-language* "bash" "sh")
(put *extension-language* "zsh" "sh")
(put *extension-language* "lua" "lua")
(put *extension-language* "java" "java")
(put *extension-language* "kt" "kotlin")
(put *extension-language* "swift" "swift")
(put *extension-language* "hs" "haskell")
(put *extension-language* "ex" "elixir")
(put *extension-language* "exs" "elixir")
(put *extension-language* "erl" "erlang")
(put *extension-language* "nim" "nim")
(put *extension-language* "zig" "zig")
(put *extension-language* "nix" "nix")
(put *extension-language* "clj" "clojure")
(put *extension-language* "el" "emacs-lisp")
(put *extension-language* "lisp" "lisp")
(put *extension-language* "scm" "scheme")
(put *extension-language* "fish" "fish")
(put *extension-language* "sql" "sql")
(put *extension-language* "toml" "toml")
(put *extension-language* "yaml" "yaml")
(put *extension-language* "yml" "yaml")
(put *extension-language* "json" "json")
(put *extension-language* "html" "html")
(put *extension-language* "htm" "html")
(put *extension-language* "css" "css")
(put *extension-language* "scss" "css")
(put *extension-language* "xml" "xml")
(put *extension-language* "md" "markdown")
(put *extension-language* "markdown" "markdown")
(put *extension-language* "org" "org")
(put *extension-language* "tf" "terraform")
(put *extension-language* "proto" "protobuf")

# --- Comment markers per language --------------------------------------

# Mutate the table created in syntax_core.janet — never use def here.
(put *comment-markers* "janet" "#")
(put *comment-markers* "rust" "//")
(put *comment-markers* "python" "#")
(put *comment-markers* "javascript" "//")
(put *comment-markers* "typescript" "//")
(put *comment-markers* "go" "//")
(put *comment-markers* "c" "//")
(put *comment-markers* "cpp" "//")
(put *comment-markers* "ruby" "#")
(put *comment-markers* "sh" "#")
(put *comment-markers* "lua" "--")
(put *comment-markers* "java" "//")
(put *comment-markers* "kotlin" "//")
(put *comment-markers* "swift" "//")
(put *comment-markers* "haskell" "--")
(put *comment-markers* "elixir" "#")
(put *comment-markers* "erlang" "%")
(put *comment-markers* "nim" "#")
(put *comment-markers* "zig" "//")
(put *comment-markers* "nix" "#")
(put *comment-markers* "clojure" ";")
(put *comment-markers* "emacs-lisp" ";")
(put *comment-markers* "lisp" ";")
(put *comment-markers* "scheme" ";")
(put *comment-markers* "fish" "#")
(put *comment-markers* "sql" "--")
(put *comment-markers* "toml" "#")
(put *comment-markers* "yaml" "#")
(put *comment-markers* "terraform" "#")
(put *comment-markers* "protobuf" "//")
(put *comment-markers* "org" "#")

(print "syntax language extensions loaded")
