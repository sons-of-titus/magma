# Rust language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about Rust: highlighting, completion, LSP,
# major mode, formatting, debugging, run configurations, project
# detection, and project templates.
(plugin/define
  {:name "rust"
   :version "0.2.0"
   :entry "main.janet"
   :languages ["rust"]
   :extensions {"rust" ["rs"]}
   :lsp {"rust" {:markers ["Cargo.toml"]}}
   :toolchain {:id "rust.cargo"
               :language "rust"
               :detect ["Cargo.toml"]
               :debug {:adapter "codelldb"}
               :actions {"build"  {:program "cargo" :args ["build"]}
                         "test"   {:program "cargo" :args ["test"]}
                         "run"    {:program "cargo" :args ["run"] :debuggable true}
                         "clean"  {:program "cargo" :args ["clean"]}
                         "format" {:program "cargo" :args ["fmt"]}
                         "lint"   {:program "cargo" :args ["clippy"]}}}
   :capabilities [:process-execution :editor-access]})
