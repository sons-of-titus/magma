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

/// Expand a snippet by trigger word.
/// Returns `Some(expanded_text)` if the trigger matches a known snippet, or `None`.
pub fn expand(trigger: &str) -> Option<String> {
    builtin().get(trigger).map(|s| s.to_string())
}

/// Expand LSP snippet syntax into plain text plus tab stops whose offsets
/// point into the *expanded* result — what the buffer must insert and where
/// the cursor cycles, with no `$`/`${}` markers left behind.
///
/// Handled: `$N`, `${N}`, `${N:placeholder}`, `$$` as a literal dollar.
/// Placeholder bodies are taken literally (no nested tab stops).
pub fn expand_lsp(src: &str) -> (String, Vec<TabStop>) {
    let mut out = String::with_capacity(src.len());
    let mut stops = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'$' {
                out.push('$');
                i += 2;
                continue;
            }
            if bytes[i + 1].is_ascii_digit() {
                let mut end = i + 1;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                let number = src[i + 1..end].parse::<usize>().unwrap_or(0);
                stops.push(TabStop {
                    number,
                    offset: out.len(),
                    text: String::new(),
                    text_len: 0,
                });
                i = end;
                continue;
            }
            if bytes[i + 1] == b'{' {
                if let Some(rel) = src[i + 2..].find('}') {
                    let inner = &src[i + 2..i + 2 + rel];
                    let (num_str, text) = match inner.find(':') {
                        Some(c) => (&inner[..c], &inner[c + 1..]),
                        None => (inner, ""),
                    };
                    if let Ok(number) = num_str.parse::<usize>() {
                        let offset = out.len();
                        out.push_str(text);
                        stops.push(TabStop {
                            number,
                            offset,
                            text: text.to_string(),
                            text_len: text.len(),
                        });
                        i += 2 + rel + 1;
                        continue;
                    }
                }
            }
        }
        let ch = src[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, stops)
}

/// The built-in snippets common to many languages, keyed by trigger.
pub fn builtin() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert(
        "for",
        "for (${1:i} = 0; ${1:i} < ${2:n}; ${1:i}++) {\n    ${0}\n}",
    );
    m.insert("if", "if (${1:condition}) {\n    ${0}\n}");
    m.insert("else", "} else {\n    ${0}\n}");
    m.insert("fun", "function ${1:name}(${2:args}) {\n    ${0}\n}");
    m.insert("def", "${1:name} = ${2:value}");
    m.insert(
        "main",
        "int main(int argc, char *argv[]) {\n    ${0}\n    return 0;\n}",
    );
    m.insert("class", "class ${1:Name} {\npublic:\n    ${1:Name}() {}\n    ~${1:Name}() {}\nprivate:\n    ${0}\n};");
    m.insert("struct", "struct ${1:Name} {\n    ${0}\n};");
    m.insert("enum", "enum ${1:Name} {\n    ${0}\n};");
    m.insert(
        "pubfn",
        "pub fn ${1:name}(${2:args}) -> ${3:Type} {\n    ${0}\n}",
    );
    m.insert("impl", "impl ${1:Name} {\n    ${0}\n}");
    m.insert("match", "match ${1:expr} {\n    ${2:pattern} => ${0},\n}");
    m.insert("println", "println!(\"${1}\");");
    m.insert("tfn", "fn ${1:name}(${2:args}) -> ${3:Type} {\n    ${0}\n}");
    m
}
