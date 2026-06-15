# Command System Design

## Philosophy

All editor actions are commands. Commands are first-class objects that can be:

- Registered from Rust or Janet
- Called programmatically from either layer
- Bound to key sequences
- Inspected at runtime
- Composed into complex operations

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                   CommandRegistry                      │
│                                                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │                 Command Table                      │  │
│  │  "save-buffer"     → Command { impl, args, doc }  │  │
│  │  "open-file"       → Command { impl, args, doc }  │  │
│  │  "quit"            → Command { impl, args, doc }  │  │
│  │  "format-buffer"   → Command { impl, args, doc }  │  │
│  │  "my-custom-cmd"   → Command { impl, args, doc }  │  │
│  └──────────────────────────────────────────────────┘  │
│                                                        │
│  ┌──────────────────────────────────────────────────┐  │
│  │             Argument Resolution                    │  │
│  │  1. Explicit args from call                       │  │
│  │  2. Interactive prompt (if interactive)           │  │
│  │  3. Default from spec                             │  │
│  │  4. Error if required and missing                 │  │
│  └──────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

## Command Definition (Rust)

```rust
#[command("save-buffer", "Save current buffer to disk")]
fn save_buffer(ctx: &mut CommandContext, args: Args) -> Result<()> {
    let buf_id = args.get_or("buffer", BufferId::current());
    let path = args.require::<PathBuf>("path")?;
    let buffer = ctx.editor.buffers.get(buf_id)?;
    ctx.editor.fs.write(&path, buffer.content())?;
    ctx.editor.events.emit("buffer-saved", table! {
        "buffer-id": buf_id,
        "path": path
    });
    Ok(())
}
```

### Command Registration

```rust
pub struct Command {
    pub name: String,
    pub description: String,
    pub args: Vec<ArgSpec>,
    pub handler: Box<dyn Fn(&mut CommandContext, Args) -> Result<()>>,
}

pub struct ArgSpec {
    pub name: String,
    pub type_: ArgType,
    pub required: bool,
    pub default: Option<ArgValue>,
    pub prompt: Option<String>,  // Interactive prompt text
}

pub enum ArgType {
    String,
    Integer,
    Boolean,
    Buffer,
    Window,
    KeySequence,
    Path,
    Any,
}
```

## Command Definition (Janet)

### Simple Command

```janet
(command/define "hello"
  (fn [& args]
    (print "Hello, Magma!"))
  {:doc "Print a greeting"})
```

### Command with Arguments

```janet
(command/define "insert-at-cursor"
  (fn [& args]
    (def text (get args :text))
    (def buf (or (get args :buffer) (buffer/current)))
    (buffer/insert buf (buffer/cursor buf) text))
  {:args [{:name :text :type :string :required true}
          {:name :buffer :type :buffer :optional true}]
   :doc "Insert text at cursor position"})
```

### Command from Existing Function

```janet
(defn my-save-hook []
  (print "Saving...")
  (trim-trailing-whitespace))

(command/define "my-save" my-save-hook)
```

## Command Dispatch

### From Rust

```rust
editor.commands.run("save-buffer", args)?;
```

### From Janet

```janet
(command/run "save-buffer")
(command/run "save-buffer" {:path "/tmp/test.txt"})
(command/run "insert-at-cursor" {:text "hello"})
```

### From Keybinding

```janet
(keymap/set "ctrl-s" "save-buffer")
```

## Argument Resolution

When a command is called, arguments are resolved in this order:

1. **Explicit arguments** — passed in the call
2. **Interactive prompts** — if the command is called interactively (from a keybinding or command palette) and args are missing, the user is prompted:
   - String: inline input in the minibuffer
   - Buffer: list of buffers to choose from
   - Path: file browser
   - Integer: numeric input
3. **Defaults** — from the `ArgSpec`
4. **Error** — required args with no value produce an error

## Command Interception

Commands can be intercepted via hooks:

```janet
(hook/add :command :before-dispatch
  (fn [cmd-name args]
    (when (= cmd-name "quit")
      (confirm-unsaved-buffers))))

(hook/add :command :after-dispatch
  (fn [cmd-name result]
    (print "Ran command: " cmd-name)))
```

## Command Palette

The command palette lists all registered commands and allows interactive invocation:

```janet
(command/run "command-palette")  # Opens interactive command selector
```

It is implemented in Janet using the event system and buffer API:

```janet
(defn command-palette []
  (def palette-buf (buffer/create "*palette*"))
  (buffer/insert palette-buf 0 (string/join (command/list) "\n"))
  ;; Filter-as-you-type, select with enter
  ...)
```

## Introspection

```janet
# List all commands
(command/list)
=> @["save-buffer" "open-file" "quit" "format-buffer" ...]

# Describe a command
(describe command/save-buffer)
=> {:name "save-buffer" :doc "Save current buffer to disk"
    :args @[{:name "buffer" :type :buffer :optional true}]
    :source :rust}

# Check if command exists
(command/exists? "save-buffer")
=> true
```

## Built-in Commands

The Rust core provides minimal built-in commands:

| Command | Args | Description |
|---------|------|-------------|
| `save-buffer` | `path?`, `buffer?` | Save buffer to disk |
| `open-file` | `path` | Open file into buffer |
| `quit` | — | Exit the editor |
| `eval` | `expr` | Evaluate Janet expression |
| `buffer-next` | — | Switch to next buffer |
| `buffer-prev` | — | Switch to previous buffer |
| `split-window-h` | — | Split window horizontally |
| `split-window-v` | — | Split window vertically |
| `close-window` | — | Close current window |
| `scroll-up` | `count?` | Scroll window up |
| `scroll-down` | `count?` | Scroll window down |
| `command-palette` | — | Open command palette |

All other commands are defined in Janet.
