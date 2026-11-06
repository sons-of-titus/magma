//! Shared agent/LLM vocabulary: tool schema, system prompt, context
//! shaping, and response-size guards.  Protocol-specific request building
//! and parsing live in `translate`; provider knowledge lives in `registry`
//! and `providers`.

use serde_json::{Value, json};

pub const MAX_BUFFER_CONTEXT_CHARS: usize = 8_000;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

pub fn tools_schema() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Run a shell/terminal command in the project root directory. Returns exit code, stdout, and stderr.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The exact shell command line to execute (e.g. 'cargo test', 'git status', 'ls -la')."
                        }
                    },
                    "required": ["command"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the text contents of a file in the workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file relative to the project root or absolute."
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional 1-based start line number to begin reading from."
                        },
                        "line_count": {
                            "type": "integer",
                            "description": "Optional maximum number of lines to read."
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write or overwrite a file in the workspace with new content.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file relative to the project root or absolute."
                        },
                        "content": {
                            "type": "string",
                            "description": "The full content to write to the file."
                        }
                    },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "edit_file",
                "description": "Replace a specific target text block in a file with new replacement text.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file relative to the project root or absolute."
                        },
                        "target": {
                            "type": "string",
                            "description": "The exact existing text to replace."
                        },
                        "replacement": {
                            "type": "string",
                            "description": "The new replacement text."
                        }
                    },
                    "required": ["path", "target", "replacement"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_directory",
                "description": "List files and directories at a given path in the workspace.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path relative to project root (defaults to '.')."
                        }
                    },
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_active_buffer",
                "description": "Read the text of the currently focused buffer (truncated).",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "get_diagnostics",
                "description": "Read the current diagnostics (errors/warnings) for the focused buffer.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_editor_commands",
                "description": "List the editor commands available to run_editor_command.",
                "parameters": { "type": "object", "properties": {}, "required": [] }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_editor_command",
                "description": "Run an editor command, e.g. {\"command\":\"open-file\",\"args\":{\"path\":\"src/main.rs\"}} or {\"command\":\"save-buffer\"}.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "args": {
                            "type": "object",
                            "description": "String-valued command arguments; omit for commands without arguments.",
                            "additionalProperties": { "type": "string" }
                        }
                    },
                    "required": ["command"]
                }
            }
        }
    ])
}

pub fn system_prompt() -> String {
    "You are Magma's built-in intelligent coding agent. You collaborate with \
     the user using a realistic development workflow: exploring files, making \
     targeted edits, running shell/build commands to verify changes, and \
     inspecting diagnostics.\n\
     \n\
     Available tools:\n\
     - run_command: Run terminal/shell commands (e.g. cargo check, cargo test, git status, etc.). \
       Returns real stdout, stderr, and exit codes.\n\
     - read_file: Read workspace file content with line numbers.\n\
     - write_file: Write or overwrite full file content.\n\
     - edit_file: Perform precise search-and-replace edits in files.\n\
     - list_directory: Inspect files and directories in the project.\n\
     - read_active_buffer: Inspect the editor's currently focused buffer.\n\
     - get_diagnostics: Check compiler/LSP errors and warnings.\n\
     - run_editor_command: Dispatch editor actions (open-file, save-buffer, etc.).\n\
     \n\
     Always verify your changes by running tests or checks when appropriate. \
     Actions that modify files or execute shell commands require the user's \
     approval — never assume an unapproved action executed. Report concisely what you did."
        .to_string()
}

pub fn context_message(ctx: &crate::types::AgentContext) -> String {
    format!(
        "Editor context for this session:\n\n{}",
        ctx.to_prompt_string()
    )
}

pub fn truncate_for_context(text: &str) -> String {
    if text.chars().count() <= MAX_BUFFER_CONTEXT_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(MAX_BUFFER_CONTEXT_CHARS).collect();
    format!("{cut}\n…[truncated]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_schema_has_function_definitions() {
        let tools = tools_schema();
        let arr = tools.as_array().unwrap();
        assert!(arr.len() >= 4);
        for t in arr {
            assert_eq!(t["type"], "function");
            assert!(t["function"]["name"].is_string());
            assert!(t["function"]["parameters"].is_object());
        }
    }

    #[test]
    fn truncation_keeps_prefix_and_marks_it() {
        let long = "x".repeat(MAX_BUFFER_CONTEXT_CHARS + 100);
        let cut = truncate_for_context(&long);
        assert!(cut.contains("…[truncated]"));
        assert!(cut.chars().count() < long.chars().count());
        assert_eq!(truncate_for_context("short"), "short");
    }
}
