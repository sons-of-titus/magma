# Zig language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about zig: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "zig"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["zig"]
   :extensions {"zig" ["zig"]}
   :lsp {"zig" {:markers ["build.zig"]}}
   :toolchain {:id "zig.toolchain"
               :language "zig"
               :detect ["build.zig"]
               :actions {"build" {:program "zig" :args ["build"]}
                         "test"  {:program "zig" :args ["build" "test"]}}}
   :capabilities [:process-execution :editor-access]})
