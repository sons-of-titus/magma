/// Typed IDs for all editor objects.
/// IDs are simple u64 values. No pointer crosses the Rust↔Janet boundary.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HookId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MarkId(pub u64);

impl BufferId {
    pub fn from_u64(id: u64) -> Self {
        BufferId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl WindowId {
    pub fn from_u64(id: u64) -> Self {
        WindowId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl CommandId {
    pub fn from_u64(id: u64) -> Self {
        CommandId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl SubscriptionId {
    pub fn from_u64(id: u64) -> Self {
        SubscriptionId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl HookId {
    pub fn from_u64(id: u64) -> Self {
        HookId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl MarkId {
    pub fn from_u64(id: u64) -> Self {
        MarkId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}
