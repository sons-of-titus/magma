use std::collections::HashMap;
use crate::kernel::command::args::ArgValue;
use crate::kernel::command::CommandResult;
use super::ScriptRuntime;

pub struct NullRuntime;

impl NullRuntime {
    pub fn new() -> Self {
        NullRuntime
    }
}

impl ScriptRuntime for NullRuntime {
    fn init(&mut self, _editor: *mut crate::kernel::state::Editor) {}

    fn eval(&mut self, expr: &str) -> String {
        format!("Scripting runtime not available: {expr}")
    }

    fn eval_result(&mut self, expr: &str) -> Result<String, String> {
        Err(format!("Scripting runtime not available: {expr}"))
    }

    fn load_file(&mut self, _path: &str) -> Result<(), String> {
        Err("Scripting runtime not available".to_string())
    }

    fn call_command(
        &mut self,
        name: &str,
        _args: &HashMap<String, ArgValue>,
    ) -> CommandResult {
        Err(format!("Unknown command: {name}"))
    }
}
