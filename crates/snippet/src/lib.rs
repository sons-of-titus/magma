//! Snippet engine — tab stops, placeholders, and the built-in snippet table.
//!
//! Snippet syntax:
//!   `${1:placeholder}`   — tab stop 1 with placeholder text
//!   `$1`                 — tab stop 1 (mirror)
//!   `${1}`               — tab stop 1 (no placeholder)
//!   `$0`                 — final cursor position
//!
//! This crate is pure: it turns snippet text into tab stops and looks up
//! expansions.  Applying an expansion to a buffer is the editor's job.

#[cfg(test)]
mod tests;

use std::collections::HashMap;

/// A parsed tab stop within an expanded snippet.
#[derive(Debug, Clone)]
pub struct TabStop {
    /// The tab stop number (1-based). 0 means final cursor position.
    pub number: usize,
    /// Byte offset within the buffer where this tab stop is located.
    pub offset: usize,
    /// The placeholder/default text for this tab stop.
    pub text: String,
    /// Length of the placeholder text (for selection).
    pub text_len: usize,
}

/// Parse tab stops from an expanded snippet string.
/// Returns a list of tab stops sorted by occurrence in the string.
pub fn parse_tabstops(expanded: &str) -> Vec<TabStop> {
    let mut stops = Vec::new();
    let mut offset = 0usize;
    let bytes = expanded.as_bytes();
    let len = bytes.len();

    while offset < len {
        // Look for '$' followed by a digit or '{'
        if bytes[offset] == b'$' && offset + 1 < len {
            if bytes[offset + 1] == b'{' {
                // ${N:text} or ${N}
                let end = offset + 2;
                if let Some(close) = expanded[end..].find('}') {
                    let inner = &expanded[offset + 2..offset + 2 + close];
                    let (num_str, text) = if let Some(colon) = inner.find(':') {
                        (&inner[..colon], &inner[colon + 1..])
                    } else {
                        (inner, "")
                    };
                    if let Ok(number) = num_str.parse::<usize>() {
                        stops.push(TabStop {
                            number,
                            offset, // placeholder: will be adjusted after expansion
                            text: text.to_string(),
                            text_len: if text.is_empty() { 0 } else { expanded[offset..offset + 2 + close + 1].len() },
                        });
                    }
                    offset += 2 + close + 1;
                    continue;
                }
            } else if bytes[offset + 1].is_ascii_digit() {
                // $N
                let num_end = offset + 1;
                while num_end + 1 < len && bytes[num_end + 1].is_ascii_digit() {
                    // Just find the full number
                }
                // For simple $N, just read one digit
                if let Ok(number) = (bytes[offset + 1] - b'0').to_string().parse::<usize>() {
                    stops.push(TabStop {
                        number,
                        offset,
                        text: String::new(),
                        text_len: 2,
                    });
                }
                offset += 2;
                continue;
            }
        }
        offset += 1;
    }

    // Sort by occurrence order (first appearance in string)
    stops
}

/// Expand a snippet by trigger word.
/// Returns `Some(expanded_text)` if the trigger matches a known snippet, or `None`.
pub fn expand(trigger: &str) -> Option<String> {
    builtin().get(trigger).map(|s| s.to_string())
}

/// The built-in snippets common to many languages, keyed by trigger.
pub fn builtin() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("for", "for (${1:i} = 0; ${1:i} < ${2:n}; ${1:i}++) {\n    ${0}\n}");
    m.insert("if", "if (${1:condition}) {\n    ${0}\n}");
    m.insert("else", "} else {\n    ${0}\n}");
    m.insert("fun", "function ${1:name}(${2:args}) {\n    ${0}\n}");
    m.insert("def", "${1:name} = ${2:value}");
    m.insert("main", "int main(int argc, char *argv[]) {\n    ${0}\n    return 0;\n}");
    m.insert("class", "class ${1:Name} {\npublic:\n    ${1:Name}() {}\n    ~${1:Name}() {}\nprivate:\n    ${0}\n};");
    m.insert("struct", "struct ${1:Name} {\n    ${0}\n};");
    m.insert("enum", "enum ${1:Name} {\n    ${0}\n};");
    m.insert("pubfn", "pub fn ${1:name}(${2:args}) -> ${3:Type} {\n    ${0}\n}");
    m.insert("impl", "impl ${1:Name} {\n    ${0}\n}");
    m.insert("match", "match ${1:expr} {\n    ${2:pattern} => ${0},\n}");
    m.insert("println", "println!(\"${1}\");");
    m.insert("tfn", "fn ${1:name}(${2:args}) -> ${3:Type} {\n    ${0}\n}");
    m
}
