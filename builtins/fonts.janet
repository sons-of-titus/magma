# Font configuration helpers and default keybindings.
# Janet owns all font policy; Rust provides the storage and renderer integration.

(defn font/increase-size []
  (font/set-size (+ (font/size) 1)))

(defn font/decrease-size []
  (font/set-size (max 4 (- (font/size) 1))))

(command/define "font-increase" font/increase-size)
(command/define "font-decrease" font/decrease-size)

(keymap/set "global" "ctrl-=" "font-increase")
(keymap/set "global" "ctrl--" "font-decrease")
