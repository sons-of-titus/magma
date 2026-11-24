# Nix language plugin — IntelliJ-style bundle.
#
# One plugin owns everything about nix: highlighting, completion,
# major mode and formatting.
(plugin/define
  {:name "nix"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["nix"]
   :extensions {"nix" ["nix"]}
   :lsp {"nix" {:markers ["flake.nix" "shell.nix" "default.nix"]}}
   :capabilities [:process-execution :editor-access]})
