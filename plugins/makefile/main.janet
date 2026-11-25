# Makefile plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(semantic/register-provider "makefile" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "makefile"
  ["include" "include" "ifeq" "ifneq" "ifdef" "ifndef" "else" "endif"
   "define" "endef" "export" "unexport" "override" "vpath"
   ".PHONY" ".SUFFIXES" ".DEFAULT" ".PRECIOUS" ".INTERMEDIATE"
   ".SECONDARY" ".DELETE_ON_ERROR" ".SILENT" ".EXPORT_ALL_VARIABLES"
   ".NOTPARALLEL" ".ONESHELL" ".POSIX"]
  "#")

(syntax/map-extension "mk" "makefile")
(syntax/map-extension "mak" "makefile")

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "makefile-mode"
  {:parent "prog"
   :layers [:makefile-mode]
   :setup (fn []
            (def width (option/get "makefile-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
