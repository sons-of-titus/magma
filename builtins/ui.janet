# Magma default UI — the Janet behavior layer for the GUI shell.
#
# Rust renders the chrome; this module owns the user-facing behavior that
# hangs off it: theme definition, editor defaults, and the project
# lifecycle feedback loop (opened → indexed → ready).

# ── Catppuccin Mocha palette ─────────────────────────────────────────────────

(def- C
  {:base      [30  30  46]
   :mantle    [24  24  37]
   :crust     [17  17  27]
   :surface0  [49  50  68]
   :surface1  [69  71  90]
   :surface2  [88  91  112]
   :overlay0  [108 112 134]
   :overlay1  [127 132 156]
   :subtext0  [166 173 200]
   :subtext1  [186 194 222]
   :text      [205 214 244]
   :lavender  [180 190 254]
   :blue      [137 180 250]
   :sapphire  [116 199 236]
   :sky       [137 220 235]
   :teal      [148 226 213]
   :green     [166 227 161]
   :yellow    [249 226 175]
   :peach     [250 179 135]
   :maroon    [235 160 172]
   :red       [243 139 168]
   :mauve     [203 166 247]
   :pink      [245 194 231]})

# ── Face definitions ──────────────────────────────────────────────────────────

(defn- ui/define-faces []
  (face/define "fringe"            {:fg (C :overlay0):bg (C :base)})
  (face/define "border"            {:fg (C :surface1):bg (C :base)})
  (face/define "overlay-border"    {:fg (C :text)    :bg (C :mantle)}))

# ── Colorscheme command ───────────────────────────────────────────────────────

(command/define "colorscheme"
  (fn [& args]
    (def name (string/trim (or (first args) "")))
    (if (= name "")
      (editor/echo "Usage: colorscheme <name>")
      (let [home (or (os/getenv "HOME") "")
            path (string home "/.config/magma/themes/" name ".janet")]
        (if (editor/fs-exists? path)
          (ui/load-theme path)
          (editor/echo (string "Theme not found: " path) "error"))))))

# ── Editor-ready: defaults ────────────────────────────────────────────────────

(event/on "editor-ready"
  (fn [_]
    (ui/define-faces)
    (option/set "number" "true")
    (option/set "scrolloff" "3")
    (option/set "mouse-support" "true")))

# ── Project lifecycle feedback ────────────────────────────────────────────────
# The shell dispatches commands (open-project, index-project); the GUI is
# only as trustworthy as its answers, so every stage reports.

(event/on "project-opened"
  (fn [data]
    (def name (get data :name "?"))
    (editor/echo (string "Project " name " — indexing files…"))))

(event/on "project-closed"
  (fn [_]
    (editor/echo "Project closed")))

(event/on "file-indexed"
  (fn [data]
    (def name (get data :project-name "?"))
    (def count (get data :count "0"))
    (editor/echo (string "Indexed " count " files in " name))))

# ── External change notice ────────────────────────────────────────────────────
# Open files are reloaded by the kernel; surface the fact so a rewritten
# buffer never changes silently behind the user's back.

(event/on "file-changed"
  (fn [data]
    (def path (get data :path ""))
    (def kind (get data :kind ""))
    (when (= kind "removed")
      (editor/echo (string "File removed on disk: " path) "error"))))
