# Cmake language plugin — CMake build definitions.
(plugin/define
  {:name "cmake"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["cmake"]
   :extensions {"cmake" ["cmake"]}
   :filenames {"cmake" ["CMakeLists.txt"]}
   :lsp {"cmake" {:single-file true}}
   :capabilities [:process-execution :editor-access]})
