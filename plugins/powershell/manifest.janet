# Powershell language plugin — PowerShell scripts.
(plugin/define
  {:name "powershell"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["powershell"]
   :extensions {"powershell" ["ps1" "psm1" "psd1"]}
   :capabilities [:editor-access]})
