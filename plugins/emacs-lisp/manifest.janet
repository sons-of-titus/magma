# Emacs-lisp language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about emacs-lisp: highlighting, completion,
# major mode.
(plugin/define
  {:name "emacs-lisp"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["emacs-lisp"]
   :capabilities [:editor-access]})
