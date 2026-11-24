# C plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

# ── Completion + highlighting ────────────────────────────────────────────────
# tree-sitter serves offline parsing and symbol extraction; clangd is layered
# on for the semantic surface once a project opens.

(semantic/register-provider "c" "treesitter")
(semantic/register-provider "c" "lsp")

# ── Major mode ───────────────────────────────────────────────────────────────
# Auto-activated for .c/.h buffers by the core extension map on
# buffer-created; the keymap layer gives users a hook for C bindings.

(major-mode/define "c-mode"
  {:parent "prog"
   :layers [:c-mode]
   :setup (fn []
            (def width (option/get "c-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})

# ── Formatting ───────────────────────────────────────────────────────────────

(formatter/register "c" {:command "clang-format" :args @["-style=file"]})

# ── Build run configurations ─────────────────────────────────────────────────
# Configs appear per project type: CMake projects get configure → build →
# test (each step picks up the previous as its build-before dependency);
# bare Makefile projects build and test through make.

(event/on "project-opened"
  (fn [data]
    (def root (get data :root ""))
    (defn marker [f] (try (os/stat (string root "/" f)) ([_] nil)))
    (def build-dir (option/get "c-build-dir"))
    (cond
      (marker "CMakeLists.txt")
      (do
        (run-config/register "cmake-configure" "cmake" @["-S" "." "-B" build-dir])
        (run-config/register "build" "cmake"
                             @["--build" build-dir] nil false "cmake-configure")
        (run-config/register "test" "ctest"
                             @["--test-dir" build-dir] nil false "build"))
      (marker "Makefile")
      (do
        (run-config/register "build" "make" @[])
        (run-config/register "test" "make" @["test"])))))

