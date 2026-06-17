//! Debug Adapter Protocol (DAP) types — Phase 6.

pub type SessionId = u64;
pub type ThreadId = u64;
pub type FrameId = u64;

/// A breakpoint set on a source location.
#[derive(Debug, Clone)]
pub struct Breakpoint {
    pub id: u64,
    pub file: String,
    pub line: usize,
    pub verified: bool,
    pub condition: Option<String>,
}

/// A stack frame from the debuggee's call stack.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub id: FrameId,
    pub name: String,
    pub source: Option<String>,
    pub line: usize,
    pub column: usize,
}

/// A variable in the current scope.
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub type_name: Option<String>,
    /// Non-zero if this variable has children that can be expanded.
    pub variables_reference: i64,
}

/// A thread in the debuggee process.
#[derive(Debug, Clone)]
pub struct Thread {
    pub id: ThreadId,
    pub name: String,
}

/// A named region of debuggee memory.
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub address: String,
    pub size: usize,
    pub name: Option<String>,
}
