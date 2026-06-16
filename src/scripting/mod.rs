pub mod null_runtime;

use std::collections::HashMap;
use crate::command::args::ArgValue;
use crate::command::CommandResult;
use crate::state::Editor;

pub trait ScriptRuntime: Send + Sync {
    fn init(&mut self, editor: *mut Editor);
    fn eval(&mut self, expr: &str) -> String;
    fn eval_result(&mut self, expr: &str) -> Result<String, String>;
    fn load_file(&mut self, path: &str) -> Result<(), String>;
    fn call_command(
        &mut self,
        name: &str,
        args: &HashMap<String, ArgValue>,
    ) -> CommandResult;
    /// Execute a stored background task function by its task id.
    ///
    /// Runtime-specific: Janet uses this to resume a stored fiber.  The
    /// default implementation is a no-op.
    fn execute_stored_task(&mut self, _task_id: u64) {}
}
