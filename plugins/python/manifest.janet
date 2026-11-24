# Python language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about python: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "python"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["python"]
   :extensions {"python" ["py" "pyi" "pyw"]}
   :lsp {"python" {:markers ["pyproject.toml" "setup.py" "requirements.txt" "Pipfile" "poetry.lock"]}}
   :toolchain {:id "python.toolchain"
               :language "python"
               :detect ["pyproject.toml" "setup.py" "requirements.txt" "Pipfile" "poetry.lock"]
               :debug {:adapter "debugpy"}
               :actions {"test" {:program "python" :args ["-m" "pytest"]}
                         "run"  {:program "python" :args ["main.py"] :debuggable true}}}
   :capabilities [:process-execution :editor-access]})
