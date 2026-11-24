# Building Magma

Rust 1.95 (pinned in `rust-toolchain.toml`; rustup picks it up
automatically) plus a C toolchain and **libclang** — the Janet bridge
generates its bindings at build time.

## Common step: grammar libraries

Syntax highlighting loads per-platform shared libraries that are **not**
committed:

```sh
cargo xtask grammars --all
```

Needs network access, `git`, a C compiler, and the `tree-sitter` CLI (only
for grammars whose repository no longer vendors a generated parser — xtask
says so if it is missing). Without the libraries the editor still runs;
modes fall back to the native keyword highlighter.

## macOS

Nothing beyond Xcode's toolchain (libclang included).

## Linux

```sh
sudo apt install build-essential pkg-config libclang-dev libdbus-1-dev
```

- `pkg-config` + `libdbus-1-dev`: the Linux credential backend (secret
  service) links libdbus at build time; at runtime a running secret service
  stores API keys — without one, secrets stay session-only.
- Set `MAGMA_KEYCHAIN=off` to skip the key service entirely (session-only
  secrets); tests always run this way.

Desktop builds need the usual winit runtime libraries (X11 + Wayland
client libs), which any desktop distribution already ships. The clipboard
uses X11; a Wayland session needs XWayland.

## Windows

Use the **MSVC toolchain** (the default for `x86_64-pc-windows-msvc`):

- Visual Studio Build Tools (C++ workload) — compiles the bundled Janet
  amalgamation and SQLite.
- LLVM (provides `libclang.dll` on `PATH`) — Janet bindings generation.
- Slint's Skia renderer downloads a prebuilt binary for MSVC; the MinGW
  target has none and will not work.

## Environment

| Variable            | Meaning                                              |
|---------------------|------------------------------------------------------|
| `MAGMA_DIR`         | State directory override (default: platform data dir)|
| `MAGMA_CONFIG_DIR`  | Config directory override                            |
| `MAGMA_KEYCHAIN`    | `off` keeps API keys session-only                    |
| `MAGMA_SERVER_TOKEN`| Token for `magma connect` against `magma server`     |
