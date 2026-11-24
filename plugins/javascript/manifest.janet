# Javascript language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about javascript: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "javascript"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["javascript"]
   :extensions {"javascript" ["js" "jsx" "mjs" "cjs"]}
   :lsp {"javascript" {:markers ["package.json"] :server-key "tsserver"}}
   :toolchain {:id "javascript.node"
               :language "javascript"
               :detect ["package.json"]
               :debug {:adapter "js-debug-adapter"}
               :actions {"build" {:program "npm" :args ["run" "build"]}
                         "test"  {:program "npm" :args ["test"]}
                         "run"   {:program "npm" :args ["start"] :debuggable true}}}
   :capabilities [:process-execution :editor-access]})
