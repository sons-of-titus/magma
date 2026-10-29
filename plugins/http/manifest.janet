# Http language plugin — REST Client-style request files.
#
# One plugin owns everything about .http/.rest files: the major mode,
# request parsing and sending, response output, and highlighting.
(plugin/define
  {:name "http"
   :version "0.1.0"
   :entry "main.janet"
   :languages ["http"]
   :capabilities [:network :editor-access]})
