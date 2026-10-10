//! Symbol extraction for the project index.
//!
//! A text-level extractor: keyword-declaration patterns per language
//! family, comment lines skipped.  Deliberately not a parser — it runs on
//! every changed file without a grammar — but a real, honest index of
//! declarations (workspace symbol search, completion augmentation).

/// One indexed declaration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IndexedSymbol {
    pub name: String,
    /// Declaration keyword (`fn`, `class`, `defn`, …).
    pub kind: String,
    /// 0-based line.
    pub line: usize,
}

/// Language family for extraction pattern selection.
fn family(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["fn", "struct", "enum", "trait", "impl", "mod", "type", "const"],
        "python" => &["def", "class"],
        "javascript" | "typescript" => &["function", "class", "const", "let", "var", "interface", "type", "enum"],
        "go" => &["func", "type", "package"],
        "c" | "cpp" => &["struct", "class", "enum", "union", "namespace", "typedef"],
        "java" | "kotlin" | "swift" => &["class", "interface", "enum", "func", "fun"],
        "janet" | "clojure" | "lisp" | "scheme" => &["defn", "defmacro", "def", "defprotocol", "defrecord"],
        "ruby" => &["def", "class", "module"],
        "zig" => &["fn", "struct", "enum", "union", "error", "const"],
        "nim" => &["proc", "func", "type", "macro", "template"],
        "haskell" | "elixir" | "erlang" => &["def", "defp", "defmodule", "data"],
        "lua" => &["function", "local"],
        "sh" => &["function"],
        _ => &[],
    }
}

fn line_language(file: &str) -> &'static str {
    let ext = file.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "ts" | "tsx" => "typescript",
        "go" => "go",
        "c" | "h" => "c",
        "cc" | "cpp" | "hpp" | "hh" => "cpp",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "janet" | "jdn" => "janet",
        "clj" | "cljs" | "edn" => "clojure",
        "lisp" | "el" | "scm" => "lisp",
        "rb" => "ruby",
        "zig" => "zig",
        "nim" | "nims" => "nim",
        "hs" => "haskell",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "lua" => "lua",
        "sh" | "bash" | "zsh" => "sh",
        _ => "",
    }
}

fn comment_prefix(language: &str) -> &'static [&'static str] {
    match language {
        "janet" | "clojure" | "lisp" | "elixir" | "erlang" => &[";"],
        "python" | "sh" | "ruby" | "yaml" => &["#"],
        "haskell" => &["--"],
        "lua" => &["--"],
        "nim" => &["#"],
        _ => &["//"],
    }
}

/// Extract declaration symbols from `text`.  Lines whose first non-space
/// run starts with a comment marker are skipped.
pub fn extract(file: &str, text: &str) -> Vec<IndexedSymbol> {
    let language = line_language(file);
    let keywords = family(language);
    if keywords.is_empty() {
        return Vec::new();
    }
    let comments = comment_prefix(language);

    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim_start();
        if comments.iter().any(|c| line.starts_with(c)) {
            continue;
        }
        // Lisp forms open with `(`; the declaration keyword follows.
        let line = line.strip_prefix('(').unwrap_or(line);
        // Visibility and qualifiers: `pub`, `pub(crate)`, `pub(super)`,
        // plus Lua's `local` — the keyword comes after them.
        let mut line = line;
        for qualifier in ["pub(crate)", "pub(super)", "pub", "local"] {
            if let Some(rest) = line.strip_prefix(qualifier) {
                line = rest.trim_start();
            }
        }
        for kw in keywords {
            let Some(rest) = line.strip_prefix(kw) else { continue };
            // Private/public markers: `defn-`, `defn+`.
            let rest = rest.strip_prefix('-').or_else(|| rest.strip_prefix('+')).unwrap_or(rest);
            if !rest.starts_with(|c: char| c.is_whitespace() || c == '(') {
                continue;
            }
            let name: String = rest.trim_start()
                .trim_start_matches('(')
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '!')
                .collect();
            if name.is_empty() || name.parse::<f64>().is_ok() {
                continue;
            }
            out.push(IndexedSymbol { name, kind: (*kw).to_string(), line: i });
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_declarations_are_indexed() {
        let src = "mod inner;\nfn alpha() {}\npub fn beta(x: u32) -> u32 { x }\nstruct Gamma { a: u32 }\n// fn commented_out() {}\nlet fn_in_let = 1;";
        let syms = extract("lib.rs", src);
        let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["inner", "alpha", "beta", "Gamma"]);
        assert_eq!(syms[0].kind, "mod");
        assert_eq!(syms[1].line, 1);
    }

    #[test]
    fn python_and_janet_families() {
        let py = extract("app.py", "class Thing:\n    def method(self):\n        pass\n# def hidden()\n");
        assert_eq!(py.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["Thing", "method"]);

        let jl = extract("plugin.janet", "(defn- helper [x] x)\n(def value 1)\n# (defn commented [])\n");
        assert!(jl.iter().any(|s| s.name == "helper"));
        assert!(jl.iter().any(|s| s.name == "value"));
        assert!(!jl.iter().any(|s| s.name == "commented"));
    }

    #[test]
    fn unknown_languages_index_nothing() {
        assert!(extract("data.csv", "fn looks_like,a,b\n").is_empty());
    }

    #[test]
    fn numbers_and_garbage_are_rejected() {
        let src = "fn 123() {}\nfn (weird) {}\nstruct ____ok {}";
        let syms = extract("x.rs", src);
        assert_eq!(syms.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["weird", "____ok"]);
    }
}
