# Options reference

Options live in the string map on the editor; set them from Janet with
`(option/set "name" "value")` or from the Settings dialog (`ctrl-alt-s`, or
`ctrl-comma`).  The registry below is the source of truth for types, defaults,
and descriptions (`src/kernel/state/options.rs`) — the Settings dialog renders
it directly.  Options not listed here are treated as custom strings.

Any option can be set globally, buffer-locally (`option/set-local`), or per
project (`project/option-set`).

## Appearance

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `gui-theme` | enum `intellij-dark`/`intellij-light`/`mocha`/`latte`/`monokai`/`ayu-dark`/`ayu-light` | `intellij-dark` | Chrome palette: IntelliJ dark/light, Catppuccin mocha/latte, Monokai, or Ayu dark/light |
| `breadcrumb-enabled` | bool | `true` | Show the path breadcrumb under the tab bar |
| `minimap-enabled` | bool | `false` | Show the code minimap instead of the scrollbar |

## Editing

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `number` | bool | `true` | Show line numbers in the gutter |
| `scrolloff` | int | `3` | Lines of context kept around the cursor while scrolling |
| `mouse-support` | bool | `true` | Click, drag, and wheel support in the editor pane |

## Behavior

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `auto-save` | bool | `false` | Write modified buffers to disk every 30 seconds. **Opt-in** — the editor never rewrites your files uninvited. |

## Quit safety (not an option)

Quitting (ctrl-q, window close) refuses while modified writable buffers
exist; the Unsaved Changes dialog offers save-all/discard, and `force-quit`
overrides.  Read-only buffers (shell, grep output) never block.  See
`docs/architecture/gui-architecture.md`.
