//! Type marshalling between Janet and Rust types.
#![allow(dead_code, unused_imports, unused_variables)]

use std::collections::HashMap;
use crate::kernel::text_engine::Buffer;
use crate::kernel::state::id::{BufferId, WindowId, CommandId, SubscriptionId, HookId, MarkId};
use crate::kernel::event::EventData;

/// A marshalled value from Janet to Rust.
pub enum JanetArg {
    Integer(i64),
    Real(f64),
    Boolean(bool),
    String(String),
    Keyword(String),
    Array(Vec<JanetArg>),
    Table(HashMap<String, JanetArg>),
    Nil,
}

impl JanetArg {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            JanetArg::String(s) => Some(s),
            JanetArg::Keyword(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            JanetArg::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.as_i64().map(|i| i as u64)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JanetArg::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_keyword(&self) -> Option<&str> {
        match self {
            JanetArg::Keyword(s) => Some(s),
            _ => None,
        }
    }
}

/// Convert Rust EventData to a Janet table.
pub fn event_data_to_janet(data: &EventData) -> HashMap<String, JanetArg> {
    let mut map = HashMap::new();
    for (k, v) in data {
        map.insert(k.clone(), JanetArg::String(v.clone()));
    }
    map
}

/// Convert a Janet table to Rust EventData.
pub fn janet_to_event_data(table: HashMap<String, JanetArg>) -> EventData {
    let mut map = EventData::new();
    for (k, v) in table {
        if let Some(s) = v.as_string() {
            map.insert(k, s.to_string());
        }
    }
    map
}
