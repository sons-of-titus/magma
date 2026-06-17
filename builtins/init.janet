# Magma built-in initialization
# vim.janet is loaded before this file, so all vim keybindings and the
# editor/on-input interceptor are already active.

(print "Magma initializing...")

# The following sub-modules are registered as builtins in Rust
# (janet_bridge::init) and are loaded immediately after this file.  They
# are listed here for visibility / load-order documentation:
#
#   builtins/magma_utils.janet         (defines magma/trim, magma-api-version, etc.)
#   builtins/scroll_commands.janet
#   builtins/indent_commands.janet     (uses magma/trim defined above)
#   builtins/colon_mode.janet          (defines *colon-plugins*)
#   builtins/colon_dired.janet         (dired colon verbs, uses colon/define)
#   builtins/completion_mode.janet     (needs *colon-plugins*)
#   builtins/multi_cursor.janet
#   builtins/major_modes.janet         (needs *colon-plugins*)
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
   :bold false :italic false :underline false :strikethrough false :dim true})

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

(face/define "error-face"
  {:fg (catppuccin-mocha :red) :bg (catppuccin-mocha :base)
   :bold true :italic false :underline false :strikethrough false :dim false})

(face/define "warning-face"
  {:fg (catppuccin-mocha :peach) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

# --- Dired-specific faces (Sprint 12) ------------------------------------
# Used by dired_ui.janet for the directory browser.

(face/define "dired-directory-face"
  {:fg (catppuccin-mocha :blue) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "dired-symlink-face"
  {:fg (catppuccin-mocha :sky) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "dired-executable-face"
  {:fg (catppuccin-mocha :green) :bg (catppuccin-mocha :base)
   :bold false :italic false :underline false :strikethrough false :dim false})

(face/define "dired-marked-face"
  {:fg (catppuccin-mocha :red) :bg (catppuccin-mocha :base)
   :bold true :italic false :underline false :strikethrough false :dim false})

(face/define "dired-header-face"
  {:fg (catppuccin-mocha :mauve) :bg (catppuccin-mocha :base)
   :bold true :italic false :underline false :strikethrough false :dim false})

(face/define "dired-path-face"
  {:fg (catppuccin-mocha :mauve) :bg (catppuccin-mocha :base)
   :bold false :italic true :underline false :strikethrough false :dim false})

(face/define "dired-flagged-face"
  {:fg (catppuccin-mocha :maroon) :bg (catppuccin-mocha :base)
   :bold false :italic true :underline false :strikethrough false :dim false})

(face/define "dired-filter-face"
  {:fg (catppuccin-mocha :yellow) :bg (catppuccin-mocha :base)
   :bold false :italic true :underline false :strikethrough false :dim false})

(print "Magma ready")
