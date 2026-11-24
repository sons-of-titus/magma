# Dart language plugin — Flutter and Dart sources.
(plugin/define
  {:name "dart"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["dart"]
   :extensions {"dart" ["dart"]}
   :lsp {"dart" {:single-file true :args ["language-server"]}}
   :toolchain {:id "dart.pub"
               :language "dart"
               :detect ["pubspec.yaml"]
               :actions {"build" {:program "dart" :args ["compile" "exe"]}
                         "test"  {:program "dart" :args ["test"]}
                         "run"   {:program "dart" :args ["run"] :debuggable true}}}
   :capabilities [:process-execution :editor-access]})
