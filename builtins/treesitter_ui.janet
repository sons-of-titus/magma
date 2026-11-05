# Tree-sitter presentation: capture→face mapping and layer diagnostics.
#
# Mechanism (grammar loading, query registry, highlighting) lives in
# builtins/treesitter.janet; this file only maps query captures onto theme
# faces and exposes introspection over the resulting syntax layer.

# ── Scope → face mapping ─────────────────────────────────────────────────────
#
# Query capture names become highlight scopes; these registrations resolve
# the conventional capture names onto the theme's faces so plugin queries
# color without per-plugin wiring.  face/scope-face matches by prefix,
# longest first — dotted captures like `variable.builtin` resolve through
# their parent.

(each [scope face]
      [["comment" "comment-face"]
       ["constant" "constant-face"]
       ["error" "error-face"]
       ["function" "function-face"]
       ["method" "function-face"]
       ["keyword" "keyword-face"]
       ["module" "type-face"]
       ["operator" "operator-face"]
       ["property" "variable-face"]
       ["punctuation" "punctuation-face"]
       ["string" "string-face"]
       ["type" "type-face"]
       ["variable" "variable-face"]]
  (face/scope-face scope face))

# ── :hl-debug — introspect the syntax layer around the cursor ────────────────
#
# With the cursor on (or right after) a suspicious token, this prints the
# major mode, the spans that overlap the token from the layer the renderer
# reads, and the resolved face.  It separates "the layer is short" from
# "the renderer dropped a cell" — the two halves of a half-colored token.
(command/define "hl-debug"
  (fn [&]
    (def buf (buffer/current))
    (unless buf (break))
    (def pos (buffer/cursor buf))
    (def word (buffer/word-under-cursor buf))
    (def spans (buffer/highlights-layer buf "syntax"))
    (editor/echo (string "mode=" (buffer/major-mode buf)
                         " cursor=" pos
                         " spans=" (length spans)))
    (each [s e scope] spans
      (when (and (<= s pos) (< pos e))
        (editor/echo (string "  [" s " " e "] " (string/slice (buffer/slice buf 0 (buffer/len buf)) s (min e (buffer/len buf)))
                             " → " scope)))
      (when word
        (def ws (- pos (length word)))
        (when (and (<= ws s) (< s (+ pos 1)))
          (editor/echo (string "  near-word [" s " " e "] " scope)))))))

(print "tree-sitter ui loaded")
