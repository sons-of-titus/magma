# REPL Architecture

## Overview

The REPL (Read-Eval-Print Loop) is a regular editor buffer named `*repl*` with special keybindings and behavior. It provides live interaction with the running editor runtime.

## Architecture

```
┌──────────────────────────────────────────────────┐
│                   REPL System                      │
│                                                    │
│  ┌──────────────┐   ┌──────────────────────────┐  │
│  │  *repl*      │   │      Janet VM             │  │
│  │  Buffer      │   │                          │  │
│  │              │   │  ┌────────────────────┐  │  │
│  │  > (+ 1 2)   │───┼─→│ Parser             │  │  │
│  │  => 3        │   │  └────────┬───────────┘  │  │
│  │  > (buffer/  │   │           ▼               │  │
│  │  | current)  │   │  ┌────────────────────┐  │  │
│  │              │   │  │ Compiler           │  │  │
│  │              │   │  └────────┬───────────┘  │  │
│  │              │   │           ▼               │  │
│  │              │   │  ┌────────────────────┐  │  │
│  │              │   │  │ Eval (timeout 5s)  │  │  │
│  │              │   │  └────────┬───────────┘  │  │
│  │              │   │           ▼               │  │
│  │              │   │  ┌────────────────────┐  │  │
│  │  => result   │◄──┼──│ Print (format +    │  │  │
│  │              │   │  │        append)     │  │  │
│  │              │   │  └────────────────────┘  │  │
│  └──────────────┘   └──────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

## REPL as a Buffer

The REPL being a regular buffer means:

```
┌────────────────────────────────────────────┐
│  *repl*                               - □ × │
│                                              │
│  Magma REPL v0.1.0                          │
│  > (+ 1 2 3)                                │
│  => 6                                        │
│  > (defn greet [n] (string "Hello, " n))    │
│  => <function greet>                         │
│  > (greet "Magma")                           │
│  => "Hello, Magma"                           │
│  > (buffer/current)                          │
│  => <buffer:1 main.rs>                       │
│  > _                                         │
│                                              │
│  [Normal] [*repl*]  Ln 7 Col 4             │
└────────────────────────────────────────────┘
```

### Benefits

- **Free editing features**: undo, redo, copy, paste, search — all work on REPL input
- **Free persistence**: REPL history is just buffer content, can be saved
- **Free navigation**: scroll, search, jump between expressions
- **No special UI**: RE PL uses the same rendering pipeline as everything else

## Keybindings

The REPL buffer has its own keymap layer (`:repl`) that overlays on top of the current mode:

| Key | Action |
|-----|--------|
| `ctrl-j` | Evaluate the expression at cursor |
| `ctrl-enter` | Evaluate the expression at cursor (alternative) |
| `ctrl-p` | Previous expression in history |
| `ctrl-n` | Next expression in history |
| `ctrl-l` | Clear REPL buffer |
| `ctrl-c` | Cancel current evaluation |
| `tab` | Complete symbol at cursor |
| `shift-enter` | Insert newline (for multi-line expressions) |

## Expression Detection

When `ctrl-j` is pressed, the REPL determines which expression to evaluate:

```
> (+ 1 2
| 3)
=> 6
      ▲
      └── Cursor here → detects surrounding s-expression
```

The detection algorithm:

1. Find the s-expression containing the cursor position
2. If the expression is complete (balanced parens), evaluate it
3. If incomplete, continue reading on the next line (note the `|` prompt)
4. If at the last prompt (`>`), evaluate the most recent complete expression

## Evaluation

```rust
pub struct Repl {
    buffer_id: BufferId,
    history: Vec<String>,       // Expression history (ring buffer)
    history_pos: usize,         // Current position in history
    eval_fiber: Option<Fiber>,  // Running evaluation fiber
    env: JanetEnvironment,      // Shared environment
}

impl Repl {
    pub fn eval(&mut self, code: &str) -> Result<()> {
        let result = self.env.eval_with_timeout(code, Duration::from_secs(5))?;
        let formatted = format_result(&result);
        self.append_output(&formatted);
        self.history.push(code.to_string());
        self.emit_event(":repl-eval", code, &formatted);
        Ok(())
    }
}
```

### Eval Process

1. Read expression from buffer
2. If expression is empty or whitespace-only, skip
3. Parse and compile the Janet expression
4. Evaluate with a 5-second timeout
5. Format the result (via Janet's `describe` or `print`)
6. Append the result to the REPL buffer
7. Add the expression to history
8. Emit `:repl-eval` event

### Error Handling

```janet
# Error case
> (/ 1 0)
=> error: divide by zero
  [repl line 1, column 5]

# Syntax error
> (+ 1 2
| 3))
=> error: unexpected closing paren
  [repl line 2, column 4]

# Timeout
> (while true (print "x"))
=> error: evaluation timed out after 5 seconds
```

## History Navigation

The REPL maintains a ring buffer of evaluated expressions:

```janet
> (first-expression)
=> result1
> (second-expression)
=> result2
> (third-expression)      # Press ctrl-p twice
> (first-expression)      # Navigated back
                          # Press ctrl-n
> (second-expression)     # Forward again
```

History is stored in the buffer as text and also in a ring buffer for efficient navigation.

## Multi-line Input

When the expression parser detects an incomplete expression (unbalanced parens), the REPL displays a continuation prompt:

```
> (defn fib [n]
  | (if (<= n 1)
  |   n
  |   (+ (fib (- n 1))
  |      (fib (- n 2))))
  | )
=> <function fib>
```

## Completion

The REPL supports tab-completion for:

- Janet core functions (`defn`, `if`, `+`, etc.)
- Editor API functions (`buffer/`, `window/`, `event/`, etc.)
- Plugin-defined functions
- User-defined variables

```janet
> buff  ← press tab
> buffer/  ← completes
> buffer/c ← press tab
> buffer/current  ← completes (if unambiguous)
```

## Environment

The REPL shares the same environment as the rest of the editor:

- Globals defined in plugins are accessible in the REPL
- Functions defined in the REPL are immediately available to the editor
- State changes from the REPL persist beyond the session

```janet
# Define a command in the REPL
> (command/define "quick-command"
  (fn [&] (print "Defined in REPL!")))
=> nil

# It's immediately available
> (command/run "quick-command")
=> "Defined in REPL!"
```

## Startup

The REPL is automatically started when the editor launches:

```janet
# Built-in init (conceptual)
(defn start-repl []
  (def repl-buf (buffer/create "*repl*"))
  (window/set-buffer (window/current) repl-buf)
  (buffer/insert repl-buf 0 "Magma REPL v0.1.0\n")
  (repl/start repl-buf))
```

## API Reference

```janet
# Start the REPL (returns buffer-id)
(repl/start)
=> <buffer:42>

# Stop the REPL
(repl/stop)
=> nil

# Evaluate an expression (returns result string)
(repl/eval "(+ 1 2 3)")
=> "6"

# Get REPL history
(repl/history)
=> @["(+ 1 2)" "(buffer/current)"]

# Toggle REPL visibility
(repl/toggle)
=> nil

# Check if REPL is running
(repl/running?)
=> true
```
