use std::collections::HashMap;
use magma::kernel::command::args::ArgValue;
use magma::kernel::command::CommandResult;
use super::ScriptRuntime;

pub struct NullRuntime;

impl Default for NullRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl NullRuntime {
    pub fn new() -> Self {
        NullRuntime
    }
}

impl ScriptRuntime for NullRuntime {
    fn init(&mut self, _editor: &mut magma::kernel::state::Editor) {}

    fn eval(&mut self, _editor: &mut magma::kernel::state::Editor, expr: &str) -> String {
        format!("Scripting runtime not available: {expr}")
    }

    fn eval_result(
        &mut self,
        _editor: &mut magma::kernel::state::Editor,
        expr: &str,
    ) -> Result<String, String> {
        Err(format!("Scripting runtime not available: {expr}"))
    }

    fn load_file(&mut self, _editor: &mut magma::kernel::state::Editor, _path: &str) -> Result<(), String> {
        Err("Scripting runtime not available".to_string())
    }

    fn call_command(
        &mut self,
        _editor: &mut magma::kernel::state::Editor,
        name: &str,
        _args: &HashMap<String, ArgValue>,
    ) -> CommandResult {
        Err(format!("Unknown command: {name}"))
    }
}
