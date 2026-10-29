# Icon packs — the default pack and per-extension overrides live here;
# Rust only stores the pack and draws it.  Glyphs are codepoints in the
# bundled Nerd Font symbols font.  See `docs/api/janet-api.md` (icons/).

(def default-pack
  @{:files
    @{# ── systems / compiled ──
      "rs"        [0xE7A8 [250 179 135]]
      "c"         0xE649
      "h"         0xE649
      "cpp"       0xE646
      "cc"        0xE646
      "cxx"       0xE646
      "hpp"       0xE646
      "go"        [0xE627 [137 220 235]]
      "swift"     0xF06E5
      # ── scripting ──
      "py"        [0xE606 [137 180 250]]
      "rb"        [0xE605 [243 139 168]]
      "php"       [0xE608 [148 226 213]]
      "lua"       [0xE620 [137 180 250]]
      "pl"        0xE67E
      "pm"        0xE67E
      "jl"        [0xE606 [137 180 250]]
      "janet"     [0xF121 [166 227 161]]
      "clj"       0xE642
      "cljs"      0xE642
      "edn"       0xE642
      "ex"        [0xE7CD [166 227 161]]
      "exs"       [0xE7CD [166 227 161]]
      "erl"       0xE7B1
      "hrl"       0xE7B1
      "hs"        0xE777
      "sh"        [0xE691 [166 227 161]]
      "bash"      [0xE691 [166 227 161]]
      "zsh"       [0xE691 [166 227 161]]
      "fish"      [0xE691 [166 227 161]]
      "ps1"       0xE683
      # ── web ──
      "js"        [0xE60C [249 226 175]]
      "mjs"       [0xE60C [249 226 175]]
      "cjs"       [0xE60C [249 226 175]]
      "ts"        [0xE628 [137 220 235]]
      "tsx"       0xE625
      "jsx"       0xE625
      "vue"       [0xE6A0 [166 227 161]]
      "html"      [0xE60E [250 179 135]]
      "htm"       [0xE60E [250 179 135]]
      "css"       [0xE614 [116 199 236]]
      "scss"      [0xE603 [243 139 168]]
      "sass"      [0xE603 [243 139 168]]
      # ── data / config ──
      "json"      [0xEB0F [249 226 175]]
      "yaml"      [0xE6A8 [148 226 213]]
      "yml"       [0xE6A8 [148 226 213]]
      "toml"      [0xE615 [243 139 168]]
      "ini"       0xE615
      "cfg"       0xE615
      "conf"      0xE615
      "xml"       0xE619
      "sql"       [0xF1C0 [116 199 236]]
      # ── docs ──
      "md"        [0xE609 [205 214 244]]
      "markdown"  [0xE609 [205 214 244]]
      "txt"       0xF15C
      "pdf"       0xEAEB
      "log"       0xF4ED
      # ── binary / media ──
      "png"       [0xF021F [203 166 247]]
      "jpg"       [0xF021F [203 166 247]]
      "jpeg"      [0xF021F [203 166 247]]
      "gif"       [0xF021F [203 166 247]]
      "webp"      [0xF021F [203 166 247]]
      "svg"       [0xF021F [203 166 247]]
      "ico"       [0xF021F [203 166 247]]
      "zip"       0xF05C4
      "tar"       0xF05C4
      "gz"        0xF05C4
      "lock"      [0xE672 [108 112 134]]
      # ── vcs ──
      "patch"     [0xF02A2 [243 139 168]]
      "diff"      [0xF02A2 [243 139 168]]}
    :file-names
    @{"Makefile"    0xE673
      "makefile"    0xE673
      "GNUmakefile" 0xE673
      "Dockerfile"  [0xF21F [137 180 250]]
      "dockerfile"  [0xF21F [137 180 250]]
      "LICENSE"     [0xF495 [249 226 175]]
      "LICENSE.md"  [0xF495 [249 226 175]]
      "LICENSE.txt" [0xF495 [249 226 175]]
      "COPYING"     [0xF495 [249 226 175]]
      ".gitignore"    [0xF02A2 [243 139 168]]
      ".gitattributes" [0xF02A2 [243 139 168]]
      ".gitmodules"  [0xF02A2 [243 139 168]]
      "Cargo.lock"  [0xE672 [108 112 134]]
      "package-lock.json" [0xE672 [108 112 134]]
      "pnpm-lock.yaml" [0xE672 [108 112 134]]
      "yarn.lock"   [0xE672 [108 112 134]]}
    :default-file 0xEA7B
    :folder       0xF024B
    :folder-open  0xF0770})

(var active-spec default-pack)
(var overrides @{})

# Override keys normalize to extension slots: leading dot dropped,
# lowercased — so ".rs", "rs", and ".RS" all target one override.
(defn- norm-key [key]
  (def kl (string/ascii-lower (string key)))
  (if (string/has-prefix? "." kl) (string/slice kl 1) kl))

(defn- install [spec]
  (set active-spec spec)
  (def merged (table/clone spec))
  (def files (table/clone (or (get spec :files) {})))
  (loop [[k v] :pairs overrides]
    # :none marks a cleared override — skipped, so the base value survives.
    (when (not (= v :none))
      (put files k v)))
  (put merged :files files)
  (icons/set-pack "default" merged))

(defn icons/use-default
  "Restore the built-in default icon pack, dropping every override."
  []
  (set overrides @{})
  (install default-pack)
  # Keep the persisted option consistent or the option follower reloads
  # the previous pack on the next frame.
  (option/set "icon-pack" "default"))

(defn icons/override
  "Override the icon for a file extension — \"rs\", \".rs\", \"spec.ts\" —
  with a codepoint and optional [r g b] tint, or nil to drop the override
  and fall back to the pack default.  Exact file names go through
  `icons/set-pack` instead."
  [key glyph &opt rgb]
  (put overrides (norm-key key) (if glyph (if rgb [glyph rgb] glyph) :none))
  (install active-spec))

(install default-pack)
(command/define "icons-default" icons/use-default)
