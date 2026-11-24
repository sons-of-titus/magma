# Magma built-in initialization
# editor_keybindings.janet is loaded before this file, so the standard
# editor keybindings are already active.

(print "Magma initializing...")

# The following sub-modules are registered as builtins in Rust
# (janet_bridge::init) and are loaded immediately after this file.  They
# are listed here for visibility / load-order documentation:
#
#   builtins/magma_utils.janet         (defines magma/trim, magma-api-version, etc.)
#   builtins/scroll_commands.janet
#   builtins/indent_commands.janet     (uses magma/trim defined above)
#   builtins/multi_cursor.janet
#   builtins/major_modes.janet         (language major-mode detection)
#   builtins/icons.janet               (default file-icon pack + overrides)
#   builtins/plugin_loader.janet       (defines `require`)

# --- Default faces (Sprint 6) ---------------------------------------------
# Faces are named styles used by syntax highlighting, LSP semantic tokens,
# and any extension that needs per-token styling.
# Users override these via (face/define) in their init.janet.

# --- Catppuccin Mocha (default theme) -------------------------------------
# https://github.com/catppuccin/catppuccin

(def catppuccin-mocha
  {:rosewater [245 224 220]
   :flamingo  [242 205 205]
   :pink      [245 194 231]
   :mauve     [203 166 247]
   :red       [243 139 168]
   :maroon    [235 160 172]
   :peach     [250 179 135]
   :yellow    [249 226 175]
   :green     [166 227 161]
   :teal      [148 226 213]
   :sky       [137 220 235]
   :sapphire  [116 199 236]
   :blue      [137 180 250]
   :lavender  [180 190 254]
   :text      [205 214 244]
   :subtext0  [166 173 200]
   :overlay0  [108 112 134]
   :surface2  [88 91 112]
   :base      [30 30 46]})

(face/define "keyword-face"
  {:fg (catppuccin-mocha :mauve) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "string-face"
  {:fg (catppuccin-mocha :green) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "comment-face"
  {:fg (catppuccin-mocha :overlay0) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "type-face"
  {:fg (catppuccin-mocha :blue) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "function-face"
  {:fg (catppuccin-mocha :peach) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "variable-face"
  {:fg (catppuccin-mocha :text) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "constant-face"
  {:fg (catppuccin-mocha :yellow) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "operator-face"
  {:fg (catppuccin-mocha :sky) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "punctuation-face"
  {:fg (catppuccin-mocha :subtext0) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

# Semantic-only distinctions — faces with no tree-sitter capture today.
# Macros read apart from plain calls (sapphire vs peach), lifetimes apart
# from constants (teal vs yellow), generic parameters apart from the types
# they parameterize (lavender vs blue).
(face/define "macro-face"
  {:fg (catppuccin-mocha :sapphire) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "lifetime-face"
  {:fg (catppuccin-mocha :teal) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "type-parameter-face"
  {:fg (catppuccin-mocha :lavender) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

# IntelliJ-style semantic distinctions: identity colors above, and
# style-only faces below — same foreground, different weight — so a
# declaration reads as bold where its uses read normal, a mutable binding
# reads bold, a parameter leans italic, a trait leans italic in its own
# hue, and unsafe code never looks ordinary.
(face/define "declaration-face"
  {:fg (catppuccin-mocha :text) :bg (catppuccin-mocha :base)
   :bold true :italic false :underline false :strikethrough false :dim false})

(face/define "mutable-face"
  {:fg (catppuccin-mocha :text) :bg (catppuccin-mocha :base)
   :bold true :italic false :underline false :strikethrough false :dim false})

(face/define "parameter-face"
  {:fg (catppuccin-mocha :text) :bg (catppuccin-mocha :base)
   :bold false :italic true :underline false :strikethrough false :dim false})

(face/define "trait-face"
  {:fg (catppuccin-mocha :rosewater) :bg (catppuccin-mocha :base)
   :bold false :italic true :underline false :strikethrough false :dim false})

(face/define "field-face"
  {:fg (catppuccin-mocha :flamingo) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "unsafe-face"
  {:fg (catppuccin-mocha :maroon) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "error-face"
  {:fg (catppuccin-mocha :red) :bg (catppuccin-mocha :base)
   :bold true :italic false :underline false :strikethrough false :dim false})

(face/define "warning-face"
  {:fg (catppuccin-mocha :peach) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

# Dead code and deprecations — the semantic-highlight targets.  Unused
# code greys out exactly like comments (overlay0); deprecated code keeps
# the grey but strikes through.
(face/define "unused-face"
  {:fg (catppuccin-mocha :overlay0) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "deprecated-face"
  {:fg (catppuccin-mocha :overlay0) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough true :dim false})

(print "Magma ready")
