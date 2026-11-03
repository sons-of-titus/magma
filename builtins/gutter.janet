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

# Default fold icons — fold chevrons, baked as geometry by the glyph atlas
# (▾ open, ▸ collapsed), so they keep their size and weight whatever font
# the editor runs.
(gutter/set-fold-icons "▾" "▸" "fold-face")
