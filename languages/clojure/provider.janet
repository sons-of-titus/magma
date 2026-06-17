(semantic/register-provider "clojure" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "clojure")
              (os/stat (string root "/deps.edn"))
              (os/stat (string root "/project.clj")))
      (lsp/start "clojure-lsp" root))))

(semantic/register-provider "clojure" "lsp")

(defn clojure/extensions [] @[".clj"])

(defn clojure/debug-adapter [] nil)

(defn clojure/format [text] text)
