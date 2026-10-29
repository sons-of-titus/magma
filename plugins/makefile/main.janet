# Makefile plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

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

# ── Activation ───────────────────────────────────────────────────────────────
# Makefiles are usually extensionless, so the plugin matches the file's
# basename as well as its extension.

(def makefile-exts @["mk" "mak"])
(def makefile-names @["Makefile" "makefile" "GNUmakefile" "BSDmakefile"])

(defn- path-extension
  "File extension of `path` (after the last dot), or nil."
  [path]
  (when-let [i (last (string/find-all "." path))]
    (string/slice path (+ i 1))))

(defn- file-basename
  "Last path segment, with backslash separators normalized."
  [path]
  (def norm (string/replace-all "\\" "/" path))
  (if-let [i (last (string/find-all "/" norm))]
    (string/slice norm (+ i 1))
    norm))

(event/on "buffer-created"
  (fn [data]
    (def path (get data :path ""))
    (when (> (length path) 0)
      (def ext (path-extension path))
      (when (or (index-of ext makefile-exts)
                (index-of (file-basename path) makefile-names))
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "makefile-mode")]
          ((get entry :activate))
          (editor/run-command "makefile-mode"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "makefile-mode"
  {:parent "prog"
   :layers [:makefile-mode]
   :setup (fn []
            (def width (option/get "makefile-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
