# Terraform language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about terraform: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "terraform"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["terraform"]
   # The shipped grammar is the HCL one; its C entry point keeps the
   # upstream name (see grammars.toml).
   :grammar-symbols {"terraform" "tree_sitter_hcl"}
   :capabilities [:process-execution :editor-access]})
