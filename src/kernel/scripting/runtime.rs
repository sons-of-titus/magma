//! `ScriptRuntime` implementation backed by the Janet VM.

use std::collections::HashMap;

use crate::kernel::command::args::ArgValue;
use crate::kernel::command::CommandResult;
use crate::kernel::scripting::ScriptRuntime;

pub struct JanetRuntime;

impl JanetRuntime {
    pub fn new() -> Self {
        JanetRuntime
    }
}

impl ScriptRuntime for JanetRuntime {
    fn init(&mut self, editor: *mut crate::kernel::state::Editor) {
        super::EDITOR_PTR.with(|cell| cell.set(Some(editor)));
        super::loader::init_vm();
    }

    fn eval(&mut self, expr: &str) -> String {
        super::eval(expr)
    }

    fn eval_result(&mut self, expr: &str) -> Result<String, String> {
        super::eval_result(expr)
    }

    fn load_file(&mut self, path: &str) -> Result<(), String> {
        let content = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
        super::loader::eval_string(path, &content)
    }

    fn call_command(
        &mut self,
        name: &str,
        args: &HashMap<String, ArgValue>,
    ) -> CommandResult {
        let ed_ptr = super::EDITOR_PTR.with(|cell| cell.get())
            .expect("EDITOR_PTR not set — call init first");
        let ed = unsafe { &mut *ed_ptr };
        super::call_janet_command(ed, name, args)
    }

    fn execute_stored_task(&mut self, task_id: u64) {
        let ed_ptr = super::EDITOR_PTR.with(|cell| cell.get())
            .expect("EDITOR_PTR not set");
        let ed = unsafe { &mut *ed_ptr };
        super::execute_stored_task(ed, task_id);
    }
}
