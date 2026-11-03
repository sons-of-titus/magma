//! Database-aware SQL completion for console buffers.  Candidates come
//! from the active source's introspected metadata, resolved through the
//! statement's table aliases — `SELECT u.` offers the columns of `users`,
//! `FROM pub.` offers the tables of schema `pub`.

use crate::state::completion::CompletionItem;
use crate::state::Editor;
use db::{statement_at, NodeRef};

use crate::completion::MAX_ITEMS;

/// Candidates for the prefix the cursor sits on, given the text before it
/// in the current statement.  Empty when the buffer is not a console.
pub fn db_candidates(ed: &Editor, before: &str, prefix: &str) -> Vec<CompletionItem> {
    let Some((source_id, statement)) = console_context(ed) else {
        return Vec::new();
    };
    let Some(conn) = ed.db.connections.get(&source_id) else {
        return Vec::new();
    };

    // `a.b.` chains: resolve the deepest identifier we can.
    let chain = trailing_chain(before);
    let mut items = Vec::new();
    match chain.as_deref() {
        Some(parts) if parts.len() >= 1 && before.ends_with('.') => {
            let parts: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
            match parts.as_slice() {
                [alias] => {
                    // alias. → columns of the resolved table; unknown alias
                    // may be a schema. → its tables; last resort tables.
                    if let Some(table) = resolve_alias(&statement, alias) {
                        items.extend(columns_of(conn, &table));
                    } else if let Some(tables) = tables_of_schema(conn, alias) {
                        items = tables;
                    } else {
                        items.extend(all_tables(conn));
                    }
                }
                [schema, table] => {
                    items.extend(columns_of_qualified(conn, schema, table));
                }
                _ => {}
            }
        }
        _ => {
            // Bare word: keywords first, then every table/view we know.
            for kw in KEYWORDS {
                if kw.starts_with(&prefix.to_ascii_lowercase()) {
                    items.push(CompletionItem {
                        label: kw.to_string(),
                        insert: kw.to_string(),
                        kind: "keyword".into(),
                        ..Default::default()
                    });
                }
            }
            items.extend(all_tables(conn));
        }
    }
    items.retain(|i| prefix_matches(i, prefix));
    items.truncate(MAX_ITEMS);
    items
}

fn prefix_matches(item: &CompletionItem, prefix: &str) -> bool {
    let target = if item.filter.is_empty() {
        &item.label
    } else {
        &item.filter
    };
    target
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

/// (source id, full statement text under the cursor).  Alias resolution
/// needs the whole statement — the FROM clause may sit after the cursor.
fn console_context(ed: &Editor) -> Option<(u64, String)> {
    let key = ed.focused_buffer_key();
    let arc = ed.buffers.get(key)?;
    let (name, text) = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        if !buf.name.starts_with("*sql") {
            return None;
        }
        (buf.name.clone(), buf.slice(0, buf.len()))
    };
    let _ = name;
    let cursor = ed.focused_view()?.cursor_offset();
    let statement = statement_at(&text, cursor)
        .map(|s| s.text)
        .unwrap_or_default();
    let source = ed.db.console_source(key).or(ed.db.active)?;
    Some((source, statement))
}

/// The dotted chain the cursor sits behind: `users u WHERE u.` → ["u"].
fn trailing_chain(before: &str) -> Option<Vec<String>> {
    if !before.ends_with('.') {
        return None;
    }
    let head = &before[..before.len() - 1];
    let mut parts = Vec::new();
    let mut it = head.rsplit(|c: char| c == '.');
    loop {
        match it.next() {
            Some(part) => {
                let ident: String = part
                    .chars()
                    .rev()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '"')
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let ident = ident.replace('"', "");
                if ident.is_empty() {
                    break;
                }
                parts.push(ident);
                if parts.len() == 2 {
                    break;
                }
            }
            None => break,
        }
    }
    parts.reverse();
    (!parts.is_empty()).then_some(parts)
}

/// Map a FROM/JOIN alias to its table within the statement.
fn resolve_alias(statement: &str, alias: &str) -> Option<(String, String)> {
    let tokens: Vec<String> = tokenize(statement);
    let mut i = 0;
    while i + 1 < tokens.len() {
        let kw = tokens[i].to_ascii_lowercase();
        if (kw == "from" || kw == "join") && i + 1 < tokens.len() {
            let mut qualified = tokens[i + 1].clone();
            let mut j = i + 1;
            while j + 1 < tokens.len() && tokens[j + 1] == "." {
                qualified.push('.');
                qualified.push_str(&tokens[j + 2]);
                j += 2;
            }
            // Optional `AS alias` or bare alias.
            let next = j + 1;
            if tokens.get(next).map(|t| t.to_ascii_lowercase()).as_deref() == Some("as")
                && let Some(a) = tokens.get(next + 1)
                && a.eq_ignore_ascii_case(alias)
            {
                return Some(split_qualified(&qualified));
            }
            if let Some(a) = tokens.get(next) {
                let is_keyword = matches!(
                    a.to_ascii_lowercase().as_str(),
                    "where" | "group" | "order" | "having" | "limit" | "offset" | "inner"
                        | "left" | "right" | "full" | "cross" | "on" | "union" | "join"
                        | "set" | "values" | "returning" | "window"
                );
                if !is_keyword && a.eq_ignore_ascii_case(alias) {
                    return Some(split_qualified(&qualified));
                }
            }
        }
        i += 1;
    }
    None
}

fn split_qualified(qualified: &str) -> (String, String) {
    match qualified.split_once('.') {
        Some((schema, table)) => (schema.to_string(), table.to_string()),
        None => (String::new(), qualified.to_string()),
    }
}

fn tokenize(statement: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    for c in statement.chars() {
        if c.is_alphanumeric() || c == '_' || c == '.' || c == '"' {
            cur.push(c);
        } else if !cur.is_empty() {
            tokens.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn columns_of(conn: &db::Connection, table: &(String, String)) -> Vec<CompletionItem> {
    let node = NodeRef::Table {
        database: String::new(),
        schema: table.0.clone(),
        name: table.1.clone(),
        is_view: false,
    };
    // The connection resolves regardless of the is_view flag.
    match conn.table_detail(&node) {
        Ok(detail) => detail
            .columns
            .iter()
            .map(|c| CompletionItem {
                label: c.name.clone(),
                insert: c.name.clone(),
                kind: "column".into(),
                detail: c.data_type.to_ascii_lowercase(),
                filter: c.name.clone(),
                ..Default::default()
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn columns_of_qualified(conn: &db::Connection, schema: &str, table: &str) -> Vec<CompletionItem> {
    columns_of(conn, &(schema.to_string(), table.to_string()))
}

fn tables_of_schema(conn: &db::Connection, schema: &str) -> Option<Vec<CompletionItem>> {
    let node = NodeRef::Folder {
        database: String::new(),
        schema: schema.to_string(),
        kind: db::FolderKind::Tables,
    };
    let tables = conn.introspect(&node).ok()?;
    let items: Vec<CompletionItem> = tables
        .into_iter()
        .filter_map(|c| match c {
            db::Child::Table { name, .. } => Some(CompletionItem {
                label: name.clone(),
                insert: name,
                kind: "table".into(),
                ..Default::default()
            }),
            _ => None,
        })
        .collect();
    (!items.is_empty()).then_some(items)
}

/// Every table/view across the cached schemas — the bare-word fallback.
fn all_tables(conn: &db::Connection) -> Vec<CompletionItem> {
    let schemas = match conn.introspect(&NodeRef::Root) {
        Ok(dbs) => {
            let mut out = Vec::new();
            for db in dbs {
                let db::Child::Database { name, .. } = db else {
                    continue;
                };
                if let Ok(schemas) = conn.introspect(&NodeRef::Database(name)) {
                    out.extend(schemas);
                }
            }
            out
        }
        Err(_) => Vec::new(),
    };
    let mut items = Vec::new();
    for child in schemas {
        let db::Child::Schema { name } = child else {
            continue;
        };
        for folder in [db::FolderKind::Tables, db::FolderKind::Views] {
            let node = NodeRef::Folder {
                database: String::new(),
                schema: name.clone(),
                kind: folder,
            };
            if let Ok(tables) = conn.introspect(&node) {
                for t in tables {
                    if let db::Child::Table { name: table, .. } = t {
                        items.push(CompletionItem {
                            label: table.clone(),
                            insert: table,
                            kind: "table".into(),
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
    items
}

const KEYWORDS: &[&str] = &[
    "select", "from", "where", "insert", "into", "values", "update", "set", "delete", "create",
    "table", "view", "index", "drop", "alter", "join", "inner", "left", "right", "full", "outer",
    "cross", "on", "as", "and", "or", "not", "null", "is", "in", "between", "like", "order",
    "group", "by", "having", "limit", "offset", "union", "all", "distinct", "case", "when",
    "then", "else", "end", "with", "primary", "key", "foreign", "references", "default",
    "constraint", "unique", "exists", "count", "sum", "avg", "min", "max", "returning",
];
