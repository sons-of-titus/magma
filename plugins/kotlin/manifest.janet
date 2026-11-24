# Kotlin language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about kotlin: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "kotlin"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["kotlin"]
   :extensions {"kotlin" ["kt" "kts"]}
   :lsp {"kotlin" {:markers ["pom.xml" "build.gradle" "build.gradle.kts" "settings.gradle" "settings.gradle.kts"]}}
   :toolchains [{:id "kotlin.gradle"
                 :language "kotlin"
                 :detect ["build.gradle.kts" "settings.gradle.kts" "build.gradle" "settings.gradle"]
                 :debug {:adapter "kotlin-debug-adapter"}
                 :actions {"build" {:program "gradle" :args ["build"]}
                           "test"  {:program "gradle" :args ["test"]}
                           "run"   {:program "gradle" :args ["run"] :debuggable true}}}
                {:id "kotlin.maven"
                 :language "kotlin"
                 :detect ["pom.xml"]
                 :actions {"build" {:program "mvn" :args ["compile"]}
                           "test"  {:program "mvn" :args ["test"]}}}]
   :capabilities [:process-execution :editor-access]})
