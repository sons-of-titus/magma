//! Database tool state: the introspection cache, open data tabs, and the
//! event vocabulary the background pool delivers.  Slow work (connect,
//! node introspection, scripts, edits) runs on the background pool and
//! lands here through [`DbEvent`].

mod events;
mod state;

pub use events::DbEvent;
pub use state::{DataTab, DataTabKind, DbState, IntrospectTree, NodeState, NodeStatus};

/// Rows fetched per query page before the result is flagged truncated.
pub const FETCH_LIMIT: usize = 200;
