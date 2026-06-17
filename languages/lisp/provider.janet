(semantic/register-provider "lisp" "treesitter")

(defn lisp/extensions [] @[".lisp"])

(defn lisp/debug-adapter [] nil)

(defn lisp/format [text] text)
