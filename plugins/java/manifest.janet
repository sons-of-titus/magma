# Java language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about java: highlighting, completion,
# major mode.
(plugin/define
  {:name "java"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["java"]
   :extensions {"java" ["java"]}
   :lsp {"java" {:markers ["pom.xml" "build.gradle" "build.gradle.kts" "settings.gradle" "settings.gradle.kts"]}}
   :toolchains [{:id "java.maven"
                 :language "java"
                 :detect ["pom.xml"]
                 :actions {"build" {:program "mvn" :args ["compile"]}
                           "test"  {:program "mvn" :args ["test"]}
                           "run"   {:program "mvn" :args ["exec:java"] :debuggable true}}}
                {:id "java.gradle"
                 :language "java"
                 :detect ["build.gradle" "build.gradle.kts" "settings.gradle" "settings.gradle.kts"]
                 :actions {"build" {:program "gradle" :args ["build"]}
                           "test"  {:program "gradle" :args ["test"]}
                           "run"   {:program "gradle" :args ["run"] :debuggable true}}}]
   :capabilities [:process-execution :editor-access]})
