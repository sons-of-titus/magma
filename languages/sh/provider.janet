(semantic/register-provider "sh" "treesitter")

(event/on "project-opened"
  (fn [data]
    (def root (get data "root" ""))
    (def name (get data "name" ""))
    (when (or (= name "sh")
              (os/stat (string root "/.shellcheckrc"))
              (os/stat (string root "/shellcheckrc")))
      (lsp/start "bash-language-server" root))))

(semantic/register-provider "sh" "lsp")

(defn sh/extensions [] @[".sh" ".bash" ".zsh"])

(defn sh/debug-adapter [] nil)

(defn sh/format [text] text)
