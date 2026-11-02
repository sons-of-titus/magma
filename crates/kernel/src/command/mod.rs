pub mod args;
pub mod builtin;

use crate::state::Editor;
use args::{ArgSpec, ArgType, ArgValue};
use std::collections::HashMap;
use std::sync::Arc;

pub type CommandResult = Result<(), String>;

/// String arguments arrive from every text boundary (command line, palette,
/// Janet `editor/run-command` / `command/run`).  When the declared type is
/// numeric, parse there — the handlers only see the typed variants.
fn coerce_string_arg(value: &mut ArgValue, arg_type: &ArgType) {
    let ArgValue::String(s) = value else {
        return;
    };
    match arg_type {
        ArgType::Integer => {
            if let Ok(i) = s.parse::<i64>() {
                *value = ArgValue::Integer(i);
            }
        }
        ArgType::Buffer => {
            if let Ok(i) = s.parse::<u64>() {
                *value = ArgValue::Buffer(i);
            }
        }
        _ => {}
    }
}

pub(crate) type CommandFn =
    dyn Fn(&mut Editor, &HashMap<String, ArgValue>) -> CommandResult + Send + Sync;

pub struct CommandEntry {
    pub(crate) description: String,
    pub(crate) args: Vec<ArgSpec>,
    pub(crate) handler: Arc<CommandFn>,
}

pub struct CommandRegistry {
    commands: HashMap<String, CommandEntry>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    pub fn new() -> Self {
        CommandRegistry {
            commands: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        args: Vec<ArgSpec>,
        handler: Box<CommandFn>,
    ) {
        self.commands.insert(
            name.to_string(),
            CommandEntry {
                description: description.to_string(),
                args,
                handler: Arc::from(handler),
            },
        );
    }

    pub fn register_fn<F>(&mut self, name: &str, description: &str, args: Vec<ArgSpec>, handler: F)
    where
        F: Fn(&mut Editor, &HashMap<String, ArgValue>) -> CommandResult + Send + Sync + 'static,
    {
        self.register(name, description, args, Box::new(handler));
    }

    pub fn list(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
    }

    /// All commands as `(name, description, needs_args)` triples, sorted by
    /// name.  `needs_args` is true when the command declares a required
    /// argument without a default — callers that cannot prompt skip those.
    pub fn list_detailed(&self) -> Vec<(String, String, bool)> {
        let mut out: Vec<(String, String, bool)> = self
            .commands
            .iter()
            .map(|(name, e)| {
                (
                    name.clone(),
                    e.description.clone(),
                    e.args.iter().any(|a| a.required && a.default.is_none()),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn exists(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// Resolve and execute a command with proper argument handling.
pub fn execute_command(
    editor: &mut Editor,
    name: &str,
    args: &HashMap<String, ArgValue>,
) -> CommandResult {
    // Clone the Arc so the borrow on editor.commands ends before we call the handler.
    let (handler, resolved_args) = {
        let entry = editor.commands.get_entry(name);

        // Fall through to the script runtime when no Rust command matches.
        // The runtime is taken out so the editor borrow can move into it.
        let entry = match entry {
            Some(e) => e,
            None => {
                let mut rt = editor.runtime.take();
                let result = match &mut rt {
                    Some(rt) => rt.call_command(editor, name, args),
                    None => Err(format!("Command not found: {name}")),
                };
                editor.runtime = rt;
                return result;
            }
        };

        let mut resolved = args.clone();
        for arg in &entry.args {
            if !resolved.contains_key(&arg.name) {
                if let Some(ref default) = arg.default {
                    resolved.insert(arg.name.clone(), default.clone());
                } else if arg.required {
                    return Err(format!("Missing required argument: {}", arg.name));
                }
            } else if let Some(value) = resolved.get_mut(&arg.name) {
                coerce_string_arg(value, &arg.arg_type);
            }
        }
        (Arc::clone(&entry.handler), resolved)
    };

    // Any named command ends the typing session: a typed run is one undo
    // step until an explicit action interrupts it.
    for (_, arc) in editor.buffers.iter() {
        let mut buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        if buf.has_open_undo_session() {
            buf.close_undo_session();
        }
    }

    let focused_before = editor
        .view_tree
        .focused_window()
        .and_then(|wid| editor.view_tree.buffer(wid));
    let rev_before = focused_before
        .and_then(|id| editor.buffers.get(id))
        .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision());

    let result = handler(editor, &resolved_args);

    // Record the cursor in the change list — but only when the command
    // actually changed the focused buffer's revision.  The name blocklist
    // this replaces both missed mutations (janet-defined) and recorded
    // non-mutations (save, toggle-fold).
    if result.is_ok() {
        let changed = focused_before
            .and_then(|id| editor.buffers.get(id))
            .map(|a| a.lock().unwrap_or_else(|e| e.into_inner()).revision())
            != rev_before;
        if changed {
            let pos = editor
                .view_of_buffer(focused_before.unwrap_or(0))
                .map(|v| v.cursor_offset())
                .unwrap_or(0);
            editor.editing.change_list.push(pos);
            editor.editing.change_list_idx = editor.editing.change_list.len();
        }
    }

    result
}

/// Execute `"name rest of line"` from a single input string — the command
/// line, the palette's argument prompt, and `editor/run-command` all parse
/// through here.  Commands with declared argument specs consume the rest
/// positionally; commands without specs (Janet-defined `(fn [arg] …)`) get
/// the raw remainder as their single `"arg"` value.
pub fn execute_command_line(editor: &mut Editor, input: &str) -> CommandResult {
    let input = input.trim();
    let Some(space) = input.find(char::is_whitespace) else {
        return execute_command(editor, input, &HashMap::new());
    };
    let (name, rest) = input.split_at(space);
    let rest = rest.trim();

    let mut args: HashMap<String, ArgValue> = HashMap::new();
    if let Some(entry) = editor.commands.get_entry(name) {
        if entry.args.len() == 1 {
            // A lone argument gets the whole remainder — `shell-command git
            // status` must not split into two positional values.
            let arg = &entry.args[0];
            if rest.is_empty() {
                if let Some(ref default) = arg.default {
                    args.insert(arg.name.clone(), default.clone());
                } else if arg.required {
                    return Err(format!("Missing required argument: {}", arg.name));
                }
            } else {
                args.insert(arg.name.clone(), ArgValue::String(rest.to_string()));
            }
        } else if entry.args.is_empty() {
            if !rest.is_empty() {
                args.insert("arg".to_string(), ArgValue::String(rest.to_string()));
            }
        } else {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            for (i, arg) in entry.args.iter().enumerate() {
                if i < parts.len() {
                    args.insert(arg.name.clone(), ArgValue::String(parts[i].to_string()));
                } else if let Some(ref default) = arg.default {
                    args.insert(arg.name.clone(), default.clone());
                } else if arg.required {
                    return Err(format!("Missing required argument: {}", arg.name));
                }
            }
        }
    }
    execute_command(editor, name, &args)
}


// Make commands field accessible from execute_command
impl CommandRegistry {
    pub fn get_entry(&self, name: &str) -> Option<&CommandEntry> {
        self.commands.get(name)
    }
}
