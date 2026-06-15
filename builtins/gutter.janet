# Gutter column definitions and line-number format helpers (Sprint 11c).

# Define named columns in render order (left-to-right).
(gutter/define-column ":breakpoints"  2  "gutter-bg")
(gutter/define-column ":vcs"          1  "gutter-bg")
(gutter/define-column ":diagnostics"  1  "gutter-bg")
(gutter/define-column ":line-numbers" 0  "gutter-bg")   # width 0 = dynamic
(gutter/define-column ":folding"      1  "gutter-bg")

# Hidden by default; extensions call gutter/show-column to reveal them.
(gutter/hide-column ":breakpoints")
(gutter/hide-column ":vcs")
(gutter/hide-column ":folding")

# Default line-number format (hybrid: absolute on cursor, relative elsewhere).
(gutter/set-line-number-format "gutter-lnum-hybrid")

# Default fold icons.
(gutter/set-fold-icons "▾" "▸" "fold-face")

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
