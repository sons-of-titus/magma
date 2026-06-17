(semantic/register-provider "swift" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "swift")
              (os/stat (string root "/Package.swift")))
      (lsp/start "sourcekit-lsp" root))))

(semantic/register-provider "swift" "lsp")

(defn swift/extensions [] @[".swift"])

(defn swift/debug-adapter [] "lldb")

(defn swift/format [text]
  (def result (process/run "swift-format" @["--mode" "stdin"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
