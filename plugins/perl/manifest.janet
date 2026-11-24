# Perl language plugin — Perl scripts and modules.
(plugin/define
  {:name "perl"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["perl"]
   :extensions {"perl" ["pl" "pm" "t"]}
   :capabilities [:editor-access]})
