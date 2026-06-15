use crate::buffer::Buffer;
use crate::command::builtin;
use crate::fs::disk::DiskFileSystem;
use crate::runtime::{ProcessState, TaskState};
use crate::state::id::BufferId;
use crate::state::Editor;

fn make_editor() -> Editor {
    let mut ed = Editor::new(Box::new(DiskFileSystem::new()));
    builtin::register_builtin_commands(&mut ed);
    let id = ed.allocate_buffer_id();
    let buf = Buffer::new(BufferId(id), "test");
    let e = ed.buffers.vacant_entry();
    let k = e.key();
    e.insert(buf);
    if let Some(win) = ed.windows.focused_window_mut() {
        win.buffer_id = Some(k);
    }
    ed
}

#[test]
fn editor_processes_starts_empty() {
    let ed = make_editor();
    assert!(ed.io.processes.is_empty(),
        "Editor must start with no managed processes");
    assert_eq!(ed.io.next_process_id, 1,
        "next_process_id must start at 1");
}

#[test]
fn editor_tasks_starts_empty() {
    let ed = make_editor();
    assert!(ed.io.tasks.is_empty(),
        "Editor must start with no background tasks");
    assert_eq!(ed.io.next_task_id, 1,
        "next_task_id must start at 1");
}

#[test]
fn process_state_can_be_constructed() {
    let state = ProcessState {
        cmd: "echo hello".to_string(),
        running: true,
        pid: 12345,
        stdin: None,
    };
    assert_eq!(state.cmd, "echo hello");
    assert!(state.running);
    assert_eq!(state.pid, 12345);
}

#[test]
fn task_state_can_be_constructed() {
    let state = TaskState {
        running: false,
    };
    assert!(!state.running);
}

#[test]
fn insert_process_then_remove() {
    let mut ed = make_editor();
    let id = ed.io.next_process_id;
    ed.io.next_process_id += 1;

    ed.io.processes.insert(id, ProcessState {
        cmd: "sleep 1".to_string(),
        running: true,
        pid: 9999,
        stdin: None,
    });

    assert!(ed.io.processes.contains_key(&id));
    assert_eq!(ed.io.processes.len(), 1);

    ed.io.processes.remove(&id);
    assert!(ed.io.processes.is_empty());
}

#[test]
fn insert_task_then_remove() {
    let mut ed = make_editor();
    let id = ed.io.next_task_id;
    ed.io.next_task_id += 1;

    ed.io.tasks.insert(id, TaskState {
        running: true,
    });

    assert!(ed.io.tasks.contains_key(&id));
    assert_eq!(ed.io.tasks.len(), 1);

    ed.io.tasks.remove(&id);
    assert!(ed.io.tasks.is_empty());
}

#[test]
fn process_state_running_flag_reflects_lifecycle() {
    let mut ed = make_editor();
    let id = ed.io.next_process_id;
    ed.io.next_process_id += 1;

    ed.io.processes.insert(id, ProcessState {
        cmd: "test".to_string(),
        running: true,
        pid: 100,
        stdin: None,
    });

    assert!(ed.io.processes.get(&id).unwrap().running);

    // Simulate what ProcessExited handler does
    if let Some(state) = ed.io.processes.get_mut(&id) {
        state.running = false;
    }
    assert!(!ed.io.processes.get(&id).unwrap().running);
}

#[test]
fn next_ids_increment() {
    let mut ed = make_editor();
    let first_pid = ed.io.next_process_id;
    ed.io.next_process_id += 1;
    let second_pid = ed.io.next_process_id;
    ed.io.next_process_id += 1;
    assert_eq!(second_pid, first_pid + 1);

    let first_tid = ed.io.next_task_id;
    ed.io.next_task_id += 1;
    let second_tid = ed.io.next_task_id;
    ed.io.next_task_id += 1;
    assert_eq!(second_tid, first_tid + 1);
}
