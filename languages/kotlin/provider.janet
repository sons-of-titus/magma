(semantic/register-provider "kotlin" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "kotlin")
              (os/stat (string root "/build.gradle.kts"))
              (os/stat (string root "/settings.gradle.kts")))
      (lsp/start "kotlin-language-server" root))))

(semantic/register-provider "kotlin" "lsp")

(defn kotlin/extensions [] @[".kt"])

(defn kotlin/debug-adapter [] nil)

(defn kotlin/format [text]
  (def result (process/run "ktlint" @["--stdin"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
