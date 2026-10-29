# Dockerfile plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "dockerfile" "treesitter")
(semantic/register-provider "dockerfile" "lsp")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "dockerfile"
  ["FROM" "RUN" "CMD" "LABEL" "MAINTAINER" "EXPOSE" "ENV" "ADD" "COPY"
   "ENTRYPOINT" "VOLUME" "USER" "WORKDIR" "ONBUILD" "STOPSIGNAL"
   "HEALTHCHECK" "SHELL" "ARG" "AS"]
  "#")

(syntax/map-extension "dockerfile" "dockerfile")

# ── Activation + LSP ─────────────────────────────────────────────────────────
# Dockerfiles are usually extensionless, so the plugin matches the file's
# basename as well as its extension.  The docker language server speaks
# stdio and serves single files, so it starts with the first Dockerfile
# buffer rather than waiting for a project to open.

(def dockerfile-exts @["dockerfile"])
(def dockerfile-names @["Dockerfile" "dockerfile"])

(defn- path-extension
  "File extension of `path` (after the last dot), or nil."
  [path]
  (when-let [i (last (string/find-all "." path))]
    (string/slice path (+ i 1))))

(defn- file-basename
  "Last path segment, with backslash separators normalized."
  [path]
  (def norm (string/replace-all "\\" "/" path))
  (if-let [i (last (string/find-all "/" norm))]
    (string/slice norm (+ i 1))
    norm))

(event/on "buffer-created"
  (fn [data]
    (def path (get data :path ""))
    (when (> (length path) 0)
      (def ext (path-extension path))
      (when (or (index-of ext dockerfile-exts)
                (index-of (file-basename path) dockerfile-names))
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "dockerfile-mode")]
          ((get entry :activate))
          (editor/run-command "dockerfile-mode"))
        (lsp/start "dockerfile" (option/get "dockerfile-lsp-command")
                   (option/get "dockerfile-lsp-args"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "dockerfile-mode"
  {:parent "prog"
   :layers [:dockerfile-mode]
   :setup (fn []
            (def width (option/get "dockerfile-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
