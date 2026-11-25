# Dockerfile plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

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

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "dockerfile-mode"
  {:parent "prog"
   :layers [:dockerfile-mode]
   :setup (fn []
            (def width (option/get "dockerfile-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
