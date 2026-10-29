# Terraform language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about terraform: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "terraform"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["terraform"]
   :capabilities [:process-execution :editor-access]})
