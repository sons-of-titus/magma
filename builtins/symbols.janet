# Symbol indexing hooks — keep the semantic index alive.
#
# The Goto-Symbol dialog, symbol breadcrumb, and go-to-definition are only
# as good as `semantic/index-buffer` input.  These hooks index a buffer when
# it is created and after each save, so navigation never faces an empty
# index.  Buffers without a tree-sitter language are skipped by the kernel.

(event/on "buffer-created"
  (fn [data]
    (def raw (get data :buffer-id))
    (when raw
      (def buf-id (scan-number raw))
      (when buf-id
        # Attach the tree-sitter language first; without it the index
        # call is a no-op.
        (ts/highlight-buffer buf-id)
        (semantic/index-buffer buf-id)))))

(event/on "buffer-after-save"
  (fn [data]
    (def raw (get data :buffer-id))
    (when raw
      (when-let [buf-id (scan-number raw)]
        (semantic/index-buffer buf-id)))))

# ── Navigation: go-to-definition (tree-sitter index, LSP fallback) ───────────

(command/define "goto-definition"
  (fn [&]
    (def buf (buffer/current))
    (unless buf (break))
    (def sym (buffer/word-under-cursor buf))
    (unless sym
      (editor/echo "No symbol under cursor" "error")
      (break))
    # A tree-sitter index hit is instant and offline; without one, defer to
    # an attached LSP (async — its result is echoed when it arrives).
    (def defs (semantic/definitions sym))
    (if (> (length defs) 0)
      (do
        (def hit (in defs 0))
        (def file (if (dictionary? hit) (get hit :file) (in hit 2)))
        (def line (if (dictionary? hit) (get hit :line) (in hit 3)))
        (when (and file (not= file ""))
          (editor/run-command "open-file" file)
          (when-let [nb (buffer/current)]
            (when-let [off (buffer/line-start-offset nb (or line 0))]
              (buffer/set-cursor nb off))
            (editor/echo (string sym " → " file ":" (+ 1 (or line 0))))))
      (editor/run-command "lsp-go-to-definition")))))

(print "symbols loaded")
