pub mod args;
pub mod builtin;

use std::collections::HashMap;
use std::sync::Arc;
use crate::state::Editor;
use args::{ArgSpec, ArgValue};

pub type CommandResult = Result<(), String>;

pub(crate) struct CommandEntry {
    #[allow(dead_code)] pub(crate) name: String,
    #[allow(dead_code)] pub(crate) description: String,
    pub(crate) args: Vec<ArgSpec>,
    pub(crate) handler: Arc<dyn Fn(&mut Editor, &HashMap<String, ArgValue>) -> CommandResult + Send + Sync>,
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
        handler: Box<dyn Fn(&mut Editor, &HashMap<String, ArgValue>) -> CommandResult + Send + Sync>,
    ) {
        self.commands.insert(name.to_string(), CommandEntry {
            name: name.to_string(),
            description: description.to_string(),
            args,
            handler: Arc::from(handler),
        });
    }

    pub fn register_fn<F>(
        &mut self,
        name: &str,
        description: &str,
        args: Vec<ArgSpec>,
        handler: F,
    ) where
        F: Fn(&mut Editor, &HashMap<String, ArgValue>) -> CommandResult + Send + Sync + 'static,
    {
        self.register(name, description, args, Box::new(handler));
    }

    pub fn list(&self) -> Vec<&str> {
        self.commands.keys().map(|s| s.as_str()).collect()
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
        let entry = editor.commands.get_entry(name)
            .ok_or_else(|| format!("Command not found: {}", name))?;

        let mut resolved = args.clone();
        for arg in &entry.args {
            if !resolved.contains_key(&arg.name) {
                if let Some(ref default) = arg.default {
                    resolved.insert(arg.name.clone(), default.clone());
                } else if arg.required {
                    return Err(format!("Missing required argument: {}", arg.name));
                }
            }
        }
        (Arc::clone(&entry.handler), resolved)
    };

    let result = handler(editor, &resolved_args);

    // Record cursor position in change list for edit commands
    if result.is_ok() && is_change_command(name) {
        let buf_id = editor.windows.focused_window()
            .and_then(|wid| editor.windows.buffer(wid))
            .unwrap_or(0);
        if editor.buffers.contains(buf_id) {
            let pos = editor.buffers.get(buf_id).map(|b| b.cursor()).unwrap_or(0);
            editor.change_list.push(pos);
            editor.change_list_idx = editor.change_list.len();
        }
    }

    result
}

fn is_change_command(name: &str) -> bool {
    !name.starts_with("move-")
        && !name.starts_with("cursor-")
        && !name.starts_with("scroll-")
        && !name.starts_with("find-")
        && !name.starts_with("search-")
        && !name.starts_with("repeat-")
        && !name.starts_with("goto-")
        && !name.starts_with("enter-")
        && !name.starts_with("exit-")
        && !name.starts_with("window-")
        && name != "force-quit"
        && name != "save-and-quit"
        && name != "alternate-buffer"
        && name != "reselect-last-visual"
        && name != "undo"
        && name != "redo"
}

// Make commands field accessible from execute_command
impl CommandRegistry {
    pub(crate) fn get_entry(&self, name: &str) -> Option<&CommandEntry> {
        self.commands.get(name)
    }
}
