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
(gutter/add-provider ":exec-line")
(gutter/add-provider ":line-numbers")
(gutter/add-provider ":folding")

# Default fold icons — the same plain glyphs the gutter already uses:
# "v" marks a foldable (open) range, ">" is the debug step pointer for a
# collapsed one.  Font-rendered, so they stay small at any size.
(gutter/set-fold-icons "v" ">" "fold-face")
