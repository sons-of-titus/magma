//! Command argument specifications and values.

#[derive(Debug, Clone)]
pub enum ArgType {
    String,
    Integer,
    Boolean,
    Buffer,
    KeySequence,
    Path,
    Any,
}

#[derive(Debug, Clone)]
pub enum ArgValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Buffer(u64),
    KeySequence(String),
    Path(String),
    Any(String),
}

impl ArgValue {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ArgValue::String(s) | ArgValue::Path(s) | ArgValue::KeySequence(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            ArgValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ArgValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_buffer(&self) -> Option<u64> {
        match self {
            ArgValue::Buffer(id) => Some(*id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArgSpec {
    pub name: String,
    pub arg_type: ArgType,
    pub required: bool,
    pub default: Option<ArgValue>,
    pub prompt: Option<String>,
}

impl ArgSpec {
    pub fn new(name: &str, arg_type: ArgType) -> Self {
        ArgSpec {
            name: name.to_string(),
            arg_type,
            required: true,
            default: None,
            prompt: None,
        }
    }

    pub fn optional(name: &str, arg_type: ArgType, default: ArgValue) -> Self {
        ArgSpec {
            name: name.to_string(),
            arg_type,
            required: false,
            default: Some(default),
            prompt: None,
        }
    }

    pub fn with_prompt(mut self, prompt: &str) -> Self {
        self.prompt = Some(prompt.to_string());
        self
    }
}
