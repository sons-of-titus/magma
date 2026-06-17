use crate::tests::helpers::make_editor;
use crate::kernel::agent::{AgentManager, AgentStatus, BuiltinPlanner};
use crate::kernel::agent::context::EditorContextProvider;
use crate::kernel::agent::types::{ContextProvider, Planner, Plan};

// ── AgentManager ─────────────────────────────────────────────────────────────

#[test]
fn new_manager_is_empty() {
    let mgr = AgentManager::new();
    assert_eq!(mgr.count(), 0);
    assert!(mgr.list().is_empty());
}

#[test]
fn create_session_increments_count() {
    let mut mgr = AgentManager::new();
    mgr.create_session("first");
    mgr.create_session("second");
    assert_eq!(mgr.count(), 2);
}

#[test]
fn new_session_status_is_running() {
    let mut mgr = AgentManager::new();
    let id = mgr.create_session("test");
    assert_eq!(mgr.status(id), Some(&AgentStatus::Running));
}

#[test]
fn complete_session_changes_status() {
    let mut mgr = AgentManager::new();
    let id = mgr.create_session("task");
    mgr.complete_session(id, "all done");
    assert_eq!(mgr.status(id), Some(&AgentStatus::Completed));
    assert_eq!(mgr.result(id), Some("all done"));
}

#[test]
fn fail_session_changes_status() {
    let mut mgr = AgentManager::new();
    let id = mgr.create_session("risky");
    mgr.fail_session(id, "boom");
    assert_eq!(mgr.status(id), Some(&AgentStatus::Failed));
    assert_eq!(mgr.sessions[&id].error.as_deref(), Some("boom"));
}

#[test]
fn unknown_session_returns_none() {
    let mgr = AgentManager::new();
    assert!(mgr.status(42).is_none());
    assert!(mgr.result(42).is_none());
}

#[test]
fn list_returns_sessions_sorted_by_id() {
    let mut mgr = AgentManager::new();
    let a = mgr.create_session("alpha");
    let b = mgr.create_session("beta");
    let c = mgr.create_session("gamma");
    let ids: Vec<u64> = mgr.list().iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![a, b, c]);
}

#[test]
fn session_ids_are_unique_and_increasing() {
    let mut mgr = AgentManager::new();
    let a = mgr.create_session("a");
    let b = mgr.create_session("b");
    let c = mgr.create_session("c");
    assert!(a < b && b < c);
}

#[test]
fn result_is_none_for_running_session() {
    let mut mgr = AgentManager::new();
    let id = mgr.create_session("in progress");
    assert!(mgr.result(id).is_none());
}

#[test]
fn agent_status_as_str_values() {
    assert_eq!(AgentStatus::Running.as_str(),   "running");
    assert_eq!(AgentStatus::Completed.as_str(), "completed");
    assert_eq!(AgentStatus::Failed.as_str(),    "failed");
}

// ── BuiltinPlanner ────────────────────────────────────────────────────────────

fn empty_ctx() -> crate::kernel::agent::AgentContext {
    crate::kernel::agent::AgentContext::default()
}

#[test]
fn planner_run_prefix_produces_task_step() {
    let plan = BuiltinPlanner.plan("run:build", &empty_ctx());
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].command, "task/run");
    assert_eq!(plan.steps[0].args.get("name").map(|s| s.as_str()), Some("build"));
}

#[test]
fn planner_goto_prefix_produces_definition_step() {
    let plan = BuiltinPlanner.plan("goto:MyFn", &empty_ctx());
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].command, "semantic/definitions");
    assert_eq!(plan.steps[0].args.get("symbol").map(|s| s.as_str()), Some("MyFn"));
}

#[test]
fn planner_diagnose_prefix_produces_empty_steps() {
    let plan = BuiltinPlanner.plan("diagnose:/src/main.rs", &empty_ctx());
    assert!(plan.steps.is_empty());
    assert!(plan.rationale.contains("Diagnostics for '/src/main.rs'"));
}

#[test]
fn planner_unknown_goal_produces_summary() {
    let plan = BuiltinPlanner.plan("explain everything", &empty_ctx());
    assert!(plan.steps.is_empty());
    assert!(plan.rationale.contains("Goal: explain everything"));
}

#[test]
fn planner_rationale_is_never_empty() {
    for goal in ["run:test", "goto:X", "diagnose:y.rs", "just help"] {
        let plan = BuiltinPlanner.plan(goal, &empty_ctx());
        assert!(!plan.rationale.is_empty(), "empty rationale for {goal}");
    }
}

#[test]
fn plan_empty_constructor() {
    let p = Plan::empty("nothing to do here");
    assert!(p.steps.is_empty());
    assert_eq!(p.rationale, "nothing to do here");
}

// ── EditorContextProvider ─────────────────────────────────────────────────────

#[test]
fn context_provider_with_empty_editor() {
    let ed = make_editor();
    let ctx = EditorContextProvider.gather(&ed);
    assert!(ctx.active_buffer_path.is_none());
    assert!(ctx.active_buffer_content.is_empty());
    assert!(ctx.diagnostics.is_empty());
}

#[test]
fn context_provider_captures_buffer_content() {
    let mut ed = make_editor();
    let key = ed.create_buffer_from_str("test.rs", "fn main() {}");
    if let Some(wid) = ed.view_tree.focused_window() {
        ed.view_tree.set_buffer(wid, key);
    }
    let ctx = EditorContextProvider.gather(&ed);
    assert!(ctx.active_buffer_content.contains("fn main()"));
}

#[test]
fn context_produces_prompt_string() {
    let ctx = empty_ctx();
    let s = ctx.to_prompt_string();
    // empty context has no parts → empty string or short
    let _ = s; // just checking it doesn't panic
}

#[test]
fn context_prompt_includes_file_path() {
    let mut ctx = empty_ctx();
    ctx.active_buffer_path = Some("/foo/bar.rs".to_string());
    ctx.active_buffer_language = "rust".to_string();
    let s = ctx.to_prompt_string();
    assert!(s.contains("/foo/bar.rs"));
    assert!(s.contains("rust"));
}
