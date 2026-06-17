(semantic/register-provider "terraform" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "terraform")
              (os/stat (string root "/.terraform")))
      (lsp/start "terraform-ls" root))))

(semantic/register-provider "terraform" "lsp")

(defn terraform/extensions [] @[".tf"])

(defn terraform/debug-adapter [] nil)

(defn terraform/format [text]
  (def result (process/run "terraform" @["fmt" "-"] text))
  (if (= (get result :exit-code) 0)
    (get result :stdout text)
    text))
