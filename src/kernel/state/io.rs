/// All I/O subsystem state: terminals, processes, tasks, and network connections.
pub struct IoState {
    pub terminals: crate::kernel::terminal::TerminalManager,
    pub processes: std::collections::HashMap<u64, crate::kernel::runtime::ProcessState>,
    pub next_process_id: u64,
    pub tasks: std::collections::HashMap<u64, crate::kernel::runtime::TaskState>,
    pub next_task_id: u64,
    pub net_connections: std::collections::HashMap<u64, crate::kernel::net::NetConnection>,
    pub net_servers: std::collections::HashMap<u64, crate::kernel::net::NetServer>,
    pub next_net_id: u64,
    /// Buffer slab key → revision of saves dispatched to the background
    /// pool but not yet confirmed on disk.  A buffer stays "dirty" for the
    /// quit guard until its write lands.
    pub saves_in_flight: std::collections::HashMap<usize, u64>,
}

impl Default for IoState {
    fn default() -> Self {
        IoState {
            terminals: crate::kernel::terminal::TerminalManager::default(),
            processes: std::collections::HashMap::new(),
            next_process_id: 1,
            tasks: std::collections::HashMap::new(),
            next_task_id: 1,
            net_connections: std::collections::HashMap::new(),
            net_servers: std::collections::HashMap::new(),
            next_net_id: 1,
            saves_in_flight: std::collections::HashMap::new(),
        }
    }
}
