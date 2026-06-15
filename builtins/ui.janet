# Magma default UI — palette, faces, event wiring, and defaults.

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
  (face/define "ml-base"     {:fg (C :subtext0) :bg (C :mantle)})
  (face/define "ml-filename" {:fg (C :text)     :bg (C :surface0)})
  (face/define "ml-pos"      {:fg (C :overlay1) :bg (C :mantle)})
  (face/define "ml-sep"      {:fg (C :surface1) :bg (C :mantle)})

  (face/define "ml-normal"  {:fg (C :base) :bg (C :blue)   :bold true})
  (face/define "ml-insert"  {:fg (C :base) :bg (C :green)  :bold true})
  (face/define "ml-visual"  {:fg (C :base) :bg (C :yellow) :bold true})
  (face/define "ml-replace" {:fg (C :base) :bg (C :red)    :bold true})
  (face/define "ml-command" {:fg (C :base) :bg (C :mauve)  :bold true})
  (face/define "ml-search"  {:fg (C :base) :bg (C :sky)    :bold true})
  (face/define "ml-term"    {:fg (C :base) :bg (C :teal)   :bold true})

  (face/define "ml-sep-normal"  {:fg (C :blue)   :bg (C :surface0)})
  (face/define "ml-sep-insert"  {:fg (C :green)  :bg (C :surface0)})
  (face/define "ml-sep-visual"  {:fg (C :yellow) :bg (C :surface0)})
  (face/define "ml-sep-replace" {:fg (C :red)    :bg (C :surface0)})
  (face/define "ml-sep-command" {:fg (C :mauve)  :bg (C :surface0)})
  (face/define "ml-sep-search"  {:fg (C :sky)    :bg (C :surface0)})
  (face/define "ml-sep-term"    {:fg (C :teal)   :bg (C :surface0)})
  (face/define "ml-sep-right"   {:fg (C :surface0) :bg (C :mantle)})

  (face/define "tab-bar"     {:fg (C :overlay0) :bg (C :crust)})
  (face/define "tab-active"  {:fg (C :text)     :bg (C :base)   :bold true})
  (face/define "tab-inactive"{:fg (C :overlay0) :bg (C :crust)})
  (face/define "tab-sep"     {:fg (C :surface0) :bg (C :crust)})

  (face/define "modeline-active"   {:fg (C :text)    :bg (C :surface1) :bold true})
  (face/define "modeline-inactive" {:fg (C :overlay0):bg (C :surface0)})
  (face/define "fringe"            {:fg (C :overlay0):bg (C :base)})
  (face/define "border"            {:fg (C :surface1):bg (C :base)})
  (face/define "overlay-border"    {:fg (C :text)    :bg (C :mantle)}))

# ── Event wiring ──────────────────────────────────────────────────────────────

(event/on "project-opened" (fn [_] (detect-branch)))
(event/on "project-closed" (fn [_] (set *ml-branch* nil)))

(event/on "render-frame"
  (fn [_]
    (apply-scroll-policy)
    (draw-modeline)))

(event/on "render-tab-bar" (fn [_] (draw-tab-bar)))

# ── :colorscheme colon verb ───────────────────────────────────────────────────

(when (and (table? *colon-plugins*) (not (nil? *colon-plugins*)))
  (put *colon-plugins* "colorscheme"
    (fn [args]
      (let [name (string/trim (if (string? args) args (string/join args " ")))]
        (if (= name "")
          (editor/log-message "Usage: :colorscheme <name>")
          (let [home (or (os/getenv "HOME") "")
                path (string home "/.config/magma/themes/" name ".janet")]
            (if (editor/fs-exists? path)
              (ui/load-theme path)
              (editor/log-message (string "Theme not found: " path)))))))))

# ── Editor-ready: defaults ────────────────────────────────────────────────────

(event/on "editor-ready"
  (fn [_]
    (ui/define-faces)
    (detect-branch)
    (option/set "number" "true")
    (option/set "scrolloff" (string *scroll-amount*))
    (ui/set-tab-bar true)
    (editor/log-message "Magma UI ready (tab-bar, modeline, line-numbers)")))
