# Swift language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about swift: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "swift"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["swift"]
   :extensions {"swift" ["swift"]}
   :lsp {"swift" {:markers ["Package.swift"]}}
   :toolchain {:id "swift.package"
               :language "swift"
               :detect ["Package.swift"]
               :actions {"build" {:program "swift" :args ["build"]}
                         "test"  {:program "swift" :args ["test"]}
                         "run"   {:program "swift" :args ["run"] :debuggable true}}}
   :capabilities [:process-execution :editor-access]})
