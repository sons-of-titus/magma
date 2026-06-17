(semantic/register-provider "emacs-lisp" "treesitter")

(defn emacs-lisp/extensions [] @[".el"])

(defn emacs-lisp/debug-adapter [] nil)

(defn emacs-lisp/format [text] text)
