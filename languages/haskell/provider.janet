(semantic/register-provider "haskell" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "haskell")
              (os/stat (string root "/cabal.project"))
              (os/stat (string root "/stack.yaml")))
      (lsp/start "haskell-language-server" root))))

(semantic/register-provider "haskell" "lsp")

(defn haskell/extensions [] @[".hs"])

(defn haskell/debug-adapter [] nil)

(defn haskell/format [text]
  (def result (process/run "fourmolu" @[] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
