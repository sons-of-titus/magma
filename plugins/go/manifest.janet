# Go language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about go: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "go"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["go"]
   :extensions {"go" ["go"]}
   :lsp {"go" {:markers ["go.mod"]}}
   :toolchain {:id "go.toolchain"
               :language "go"
               :detect ["go.mod"]
               :debug {:adapter "dlv-dap"}
               :actions {"build" {:program "go" :args ["build" "./..."]}
                         "test"  {:program "go" :args ["test" "./..."]}
                         "run"   {:program "go" :args ["run" "."] :debuggable true}
                         "clean" {:program "go" :args ["clean"]}}}
   :capabilities [:process-execution :editor-access]})
