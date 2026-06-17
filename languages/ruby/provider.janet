# Ruby Language Provider

(semantic/register-provider "ruby" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "ruby")
              (os/stat (string root "/Gemfile")))
      (lsp/start "solargraph" root))))

(semantic/register-provider "ruby" "lsp")

(defn ruby/extensions [] @[".rb"])

(defn ruby/debug-adapter [] "ruby-debug")

(defn ruby/format [text]
  (def result (process/run "rubocop" @["--auto-correct" "--stdin" "dummy.rb"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
