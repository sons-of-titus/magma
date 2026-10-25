//! `ScriptRuntime` implementation backed by the Janet VM.

use std::collections::HashMap;

use crate::kernel::command::args::ArgValue;
use crate::kernel::command::CommandResult;
use crate::kernel::scripting::ScriptRuntime;

pub struct JanetRuntime;

impl Default for JanetRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl JanetRuntime {
    pub fn new() -> Self {
        JanetRuntime
    }
}

impl ScriptRuntime for JanetRuntime {
    fn init(&mut self, editor: &mut crate::kernel::state::Editor) {
        super::scoped_editor(editor, super::loader::init_vm);
    }

    fn eval(&mut self, editor: &mut crate::kernel::state::Editor, expr: &str) -> String {
        super::scoped_editor(editor, || super::eval(expr))
    }

    fn eval_result(
        &mut self,
        editor: &mut crate::kernel::state::Editor,
        expr: &str,
    ) -> Result<String, String> {
        super::scoped_editor(editor, || super::eval_result(expr))
    }

    fn load_file(
        &mut self,
        editor: &mut crate::kernel::state::Editor,
        path: &str,
    ) -> Result<(), String> {
        super::scoped_editor(editor, || super::load_file(path))
    }

    fn call_command(
        &mut self,
        editor: &mut crate::kernel::state::Editor,
        name: &str,
        args: &HashMap<String, ArgValue>,
    ) -> CommandResult {
        super::scoped_editor(editor, || super::call_janet_command_scoped(name, args))
    }

    fn execute_stored_task(&mut self, editor: &mut crate::kernel::state::Editor, task_id: u64) {
        super::scoped_editor(editor, || super::execute_stored_task_scoped(task_id));
    }

    fn execute_task_complete_callback(
        &mut self,
        editor: &mut crate::kernel::state::Editor,
        task_id: u64,
    ) {
        super::scoped_editor(editor, || super::execute_task_complete_callback_scoped(task_id));
    }
}
