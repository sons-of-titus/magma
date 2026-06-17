# Gutter provider registrations — Phase 7.
#
# Built-in providers are registered in render order (left-to-right).
# Each call to (gutter/add-provider name) adds a named provider backed by the
# corresponding Rust GutterProvider implementation.
#
# Providers that rely on cached data (git-signs, breakpoints) are populated at
# runtime by their respective subsystem scripts or via (gutter/provider-update).

(gutter/add-provider ":breakpoints")
(gutter/add-provider ":git-signs")
(gutter/add-provider ":diagnostics")
(gutter/add-provider ":line-numbers")
(gutter/add-provider ":folding")

# Default fold icons.
(gutter/set-fold-icons "▾" "▸" "fold-face")

# Default line-number format (hybrid: absolute on cursor, relative elsewhere).
(gutter/set-line-number-format "gutter-lnum-hybrid")

# Built-in line-number format commands.
# Each receives (abs-line cursor-line total-lines) and returns a string.

(defn gutter-lnum-absolute [abs-line cursor-line total-lines]
  (string abs-line))

(defn gutter-lnum-relative [abs-line cursor-line total-lines]
  (string (math/abs (- abs-line cursor-line))))

(defn gutter-lnum-hybrid [abs-line cursor-line total-lines]
  (if (= abs-line cursor-line)
    (string abs-line)
    (string (math/abs (- abs-line cursor-line)))))

# Register them as commands so Janet extensions can refer to them by name.
(command/define "gutter-lnum-absolute" (fn [] (gutter-lnum-absolute 0 0 0)))
(command/define "gutter-lnum-relative" (fn [] (gutter-lnum-relative 0 0 0)))
(command/define "gutter-lnum-hybrid"   (fn [] (gutter-lnum-hybrid 0 0 0)))
