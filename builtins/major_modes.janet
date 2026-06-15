# --- Major modes -----------------------------------------------------------
#
# Major modes describe the content/context type of a buffer (Emacs-style).
# The Rust core knows: fundamental, text, prog.
# Language-specific modes (rust-mode, python-mode, …) are defined by
# extensions — this file only defines the core structural modes.
#
# When a major mode is activated it:
#   1. Pops the previous major mode's keymap layers.
#   2. Sets the buffer's major mode via `set-major-mode`.
#   3. Pushes the new mode's keymap layers.
#   4. Emits the `major-mode-changed` event (handled by extensions).
#
# Extensions that define language modes should follow the same pattern and
# register a colon verb in *colon-plugins* or call `major-mode/define`.

# Track which major mode layers are currently active so we can pop them.
(var *active-major-mode-layers* @[])

(defn major-mode/activate
  "Switch the focused buffer to a major mode by name.
   `layers` is a tuple of keymap layer keywords to push (parent first)."
  [mode-name layers]
  # Pop old layers
  (each layer *active-major-mode-layers*
    (keymap/pop-layer layer))
  (array/clear *active-major-mode-layers*)
  # Set Rust-side mode
  (editor/run-command "set-major-mode" mode-name)
  # Push new layers and track them
  (each layer layers
    (keymap/push-layer layer)
    (array/push *active-major-mode-layers* layer)))

# Registry of all defined major modes.
# Maps mode-name → {:parent p :layers [...] :hooks {...} :setup fn}
(def *major-modes* @{})

(defmacro major-mode/define
  "Define a major mode and register it with the extension system.

   Usage: (major-mode/define name options)
   Options table keys:
     :parent   - parent mode name (string), defaults to \"prog\"
     :layers   - keymap layer names to push when the mode activates (array)
     :hooks    - table of event-name → handler-fn pairs
     :setup    - zero-arg function called after the mode activates

   Example:
     (major-mode/define \"rust-mode\"
       {:parent \"prog\"
        :layers [:rust-mode]
        :hooks {\"buffer-before-save\" (fn [_] (editor/run-command \"format-buffer\"))}
        :setup (fn [] (option/set-local \"tab-width\" \"4\"))})"
  [name opts]
  ~(do
     (def mode-entry {:parent  (get ,opts :parent "prog")
                      :layers  (get ,opts :layers @[])
                      :hooks   (get ,opts :hooks @{})
                      :setup   (get ,opts :setup nil)})
     (put *major-modes* ,name mode-entry)
     # Register the activate command so (editor/run-command "rust-mode") works
     (command/define ,name
       (fn [&]
         (major-mode/activate ,name (get mode-entry :layers @[]))
         # Run the optional setup function
         (when (get mode-entry :setup)
           ((get mode-entry :setup)))
         # Wire up mode-specific hooks (registered once per activation)
         (eachp [ev handler] (get mode-entry :hooks @{})
           (event/on ev handler))))))

(command/define "fundamental-mode"
  (fn [&] (major-mode/activate "fundamental" [])))

(command/define "text-mode"
  (fn [&] (major-mode/activate "text" [:text-mode])))

(command/define "prog-mode"
  (fn [&] (major-mode/activate "prog" [:prog-mode])))

# --- Major mode colon verbs -----------------------------------------------

(put *colon-plugins* "fundamental-mode" (fn [_] (editor/run-command "fundamental-mode")))
(put *colon-plugins* "text-mode"        (fn [_] (editor/run-command "text-mode")))
(put *colon-plugins* "prog-mode"        (fn [_] (editor/run-command "prog-mode")))

# --- Auto-detect major mode on buffer-created -----------------------------
#
# When a file is opened via `open-file`, Rust emits `buffer-created` with a
# `path` field.  This subscriber maps common file extensions to the right mode
# command so extensions don't have to duplicate the detection logic.
#
# Language extensions may override this by subscribing to `buffer-created`
# themselves and calling (editor/run-command "their-mode") before or after
# this handler.

(defn- path-extension
  "Return the file extension from a path string (after the last dot), or nil."
  [path]
  (var last-dot nil)
  (var i 0)
  (def n (length path))
  (while (< i n)
    (when (= (string/slice path i (+ i 1)) ".")
      (set last-dot i))
    (set i (+ i 1)))
  (when last-dot
    (string/slice path (+ last-dot 1))))

(def *extension-mode-map*
  {"rs"    "rust-mode"
   "py"    "python-mode"
   "js"    "javascript-mode"
   "ts"    "typescript-mode"
   "jsx"   "javascript-mode"
   "tsx"   "typescript-mode"
   "c"     "c-mode"
   "h"     "c-mode"
   "cpp"   "cpp-mode"
   "cc"    "cpp-mode"
   "cxx"   "cpp-mode"
   "hpp"   "cpp-mode"
   "go"    "go-mode"
   "hs"    "haskell-mode"
   "lua"   "lua-mode"
   "rb"    "ruby-mode"
   "java"  "java-mode"
   "kt"    "kotlin-mode"
   "swift" "swift-mode"
   "sh"    "sh-mode"
   "bash"  "sh-mode"
   "zsh"   "sh-mode"
   "fish"  "fish-mode"
   "janet" "janet-mode"
   "jn"    "janet-mode"
   "md"    "markdown-mode"
   "markdown" "markdown-mode"
   "org"   "org-mode"
   "txt"   "text-mode"
   "toml"  "toml-mode"
   "yaml"  "yaml-mode"
   "yml"   "yaml-mode"
   "json"  "json-mode"
   "html"  "html-mode"
   "htm"   "html-mode"
   "css"   "css-mode"
   "scss"  "css-mode"
   "xml"   "xml-mode"
   "sql"   "sql-mode"
   "el"    "emacs-lisp-mode"
   "lisp"  "lisp-mode"
   "scm"   "scheme-mode"
   "clj"   "clojure-mode"
   "ex"    "elixir-mode"
   "exs"   "elixir-mode"
   "erl"   "erlang-mode"
   "nim"   "nim-mode"
   "zig"   "zig-mode"
   "nix"   "nix-mode"
   "tf"    "terraform-mode"
   "proto" "protobuf-mode"})

(event/on "buffer-created"
  (fn [data]
    (def path (get data :path ""))
    (when (> (length path) 0)
      (def ext (path-extension path))
      (when ext
        (def mode-name (get *extension-mode-map* ext nil))
        (when mode-name
          # Only activate if the mode command has been registered by a plugin
          (when (command/exists? mode-name)
            (editor/run-command mode-name)))))))
