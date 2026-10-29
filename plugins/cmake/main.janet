# Cmake plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "cmake" "treesitter")
(semantic/register-provider "cmake" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "cmake"
  ["cmake_minimum_required" "project" "subdirs" "set" "unset" "break"
   "continue" "if" "else" "elseif" "endif" "foreach" "endforeach"
   "function" "endfunction" "macro" "endmacro" "while" "endwhile"
   "return" "message" "include" "add_executable" "add_library"
   "add_subdirectory" "add_custom_target" "add_custom_command"
   "add_compile_options" "add_compile_definitions" "add_dependencies"
   "target_link_libraries" "target_include_directories"
   "target_compile_options" "target_compile_definitions"
   "target_link_options" "target_sources" "find_package" "find_library"
   "find_path" "find_program" "option" "install" "export"
   "configure_file" "file" "list" "string" "math"
   "get_filename_component" "set_property" "set_target_properties"
   "set_directory_properties" "set_source_files_properties"
   "enable_testing" "enable_language" "include_directories"
   "link_directories"]
  "#")

(syntax/map-extension "cmake" "cmake")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# CMake projects center on CMakeLists.txt, an extensionless-looking file,
# so the plugin matches the basename as well as the .cmake extension.  The
# cmake language server is project-scoped: it starts when a CMake file of
# an opened project is first edited.

(def cmake-exts @["cmake"])
(def cmake-names @["CMakeLists.txt"])

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
      (when (or (index-of ext cmake-exts)
                (index-of (file-basename path) cmake-names))
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "cmake-mode")]
          ((get entry :activate))
          (editor/run-command "cmake-mode"))
        (lsp/start "cmake" (option/get "cmake-lsp-command"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "cmake-mode"
  {:parent "prog"
   :layers [:cmake-mode]
   :setup (fn []
            (def width (option/get "cmake-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
