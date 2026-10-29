# Perl plugin entry — evaluated once at enable time with the plugin's
# capability context active, so every handler registered here carries
# those capabilities when it later fires.

(editor/load-file (string (dyn :magma/plugin-dir "") "/config.janet"))

(semantic/register-provider "perl" "treesitter")

# ── Fallback highlighting ────────────────────────────────────────────────────
# Keyword tables drive the built-in highlighter until a tree-sitter grammar
# is installed for the language.

(syntax/define-rules "perl"
  ["sub" "my" "our" "local" "use" "no" "require" "package" "if" "elsif"
   "else" "unless" "while" "until" "for" "foreach" "do" "last" "next"
   "redo" "goto" "return" "wantarray" "eval" "die" "warn" "print"
   "printf" "sprintf" "exists" "delete" "defined" "bless" "ref" "scalar"
   "keys" "values" "each" "sort" "map" "grep" "push" "pop" "shift"
   "unshift" "chomp" "chop" "split" "join" "reverse" "lc" "uc" "length"
   "substr" "index" "int" "abs" "srand" "undef"]
  "#")

(syntax/map-extension "pl" "perl")
(syntax/map-extension "pm" "perl")
(syntax/map-extension "t" "perl")

# ── Activation ───────────────────────────────────────────────────────────────
# The core extension map predates this plugin, so the plugin routes its
# own extensions: its major mode wins over the core's pick.

(def perl-exts @["pl" "pm" "t"])

(defn- path-extension
  "File extension of `path` (after the last dot), or nil."
  [path]
  (when-let [i (last (string/find-all "." path))]
    (string/slice path (+ i 1))))

(event/on "buffer-created"
  (fn [data]
    (def path (get data :path ""))
    (when (> (length path) 0)
      (def ext (path-extension path))
      (when (index-of ext perl-exts)
        # Janet-defined modes activate directly (nested fiber constraint).
        (if-let [entry (get *major-modes* "perl-mode")]
          ((get entry :activate))
          (editor/run-command "perl-mode"))))))

# ── Major mode ───────────────────────────────────────────────────────────────

(major-mode/define "perl-mode"
  {:parent "prog"
   :layers [:perl-mode]
   :setup (fn []
            (def width (option/get "perl-indent-width"))
            (option/set-local "tab-width" width)
            (option/set-local "indent-width" width))})
