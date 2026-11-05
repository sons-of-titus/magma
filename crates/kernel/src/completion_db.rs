//! Database-aware SQL completion for console buffers.  Candidates come
//! from the active source's cached catalog (loaded off-thread after
//! connect), resolved through the statement's table aliases — `SELECT u.`
//! offers the columns of `users`, `FROM pub.` offers the tables of schema
//! `pub`.  Nothing here touches the network.

use crate::state::completion::CompletionItem;
use crate::state::Editor;
use db::{statement_at, Catalog, CatalogTable, TableKind};

use crate::completion::MAX_ITEMS;

/// Candidates for the prefix the cursor sits on, given the text before it
/// in the current statement.  Empty when the buffer is not a console.
pub fn db_candidates(ed: &Editor, before: &str, prefix: &str) -> Vec<CompletionItem> {
    let Some((source_id, statement)) = console_context(ed) else {
        return Vec::new();
    };
    let catalog = ed.db.catalogs.get(&source_id).map(|c| c.as_ref());

    let mut items = Vec::new();
    match trailing_chain(before) {
        Some(parts) => {
            let Some(catalog) = catalog else {
                return Vec::new();
            };
            match parts.as_slice() {
                [name] => {
                    if let Some((schema, table)) = resolve_alias(&statement, name) {
                        items.extend(column_items(catalog, &schema, &table));
                    } else if catalog.table("", name).is_some() {
                        items.extend(column_items(catalog, "", name));
                    } else if catalog.has_schema(name) {
                        items.extend(table_items(catalog, Some(name)));
                    }
                }
                [schema, table] => items.extend(column_items(catalog, schema, table)),
                _ => {}
            }
        }
        None => {
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
            if let Some(catalog) = catalog {
                for (schema, table) in statement_tables(&statement) {
                    items.extend(column_items(catalog, &schema, &table));
                }
                items.extend(catalog.schemas.iter().map(|s| CompletionItem {
                    label: s.clone(),
                    insert: s.clone(),
                    kind: "module".into(),
                    detail: "schema".into(),
                    ..Default::default()
                }));
                items.extend(table_items(catalog, None));
            }
        }
    }
    items.retain(|i| prefix_matches(i, prefix));
    dedupe(&mut items);
    items.truncate(MAX_ITEMS);
    items
}

fn dedupe(items: &mut Vec<CompletionItem>) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|i| seen.insert((i.label.clone(), i.kind.clone(), i.detail.clone())));
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
    let text = {
        let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
        if !buf.name.starts_with("*sql") {
            return None;
        }
        buf.slice(0, buf.len())
    };
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
    for part in head.rsplit('.') {
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
    parts.reverse();
    (!parts.is_empty()).then_some(parts)
}

/// Map a FROM/JOIN alias to its table within the statement.
fn resolve_alias(statement: &str, alias: &str) -> Option<(String, String)> {
    let tokens: Vec<String> = tokenize(statement);
    for i in 0..tokens.len().saturating_sub(1) {
        let kw = tokens[i].to_ascii_lowercase();
        if kw != "from" && kw != "join" {
            continue;
        }
        let qualified = &tokens[i + 1];
        let next = i + 2;
        let candidate = match tokens.get(next).map(|t| t.to_ascii_lowercase()).as_deref() {
            Some("as") => tokens.get(next + 1),
            Some(
                "where" | "group" | "order" | "having" | "limit" | "offset" | "inner" | "left"
                | "right" | "full" | "cross" | "on" | "union" | "join" | "set" | "values"
                | "returning" | "window",
            ) => None,
            _ => tokens.get(next),
        };
        if candidate.is_some_and(|a| a.eq_ignore_ascii_case(alias)) {
            return Some(split_qualified(qualified));
        }
    }
    None
}

/// Tables named after FROM/JOIN/INTO/UPDATE in the statement.
fn statement_tables(statement: &str) -> Vec<(String, String)> {
    let tokens = tokenize(statement);
    tokens
        .windows(2)
        .filter(|w| matches!(w[0].to_ascii_lowercase().as_str(), "from" | "join" | "into" | "update"))
        .map(|w| split_qualified(&w[1]))
        .collect()
}

fn split_qualified(qualified: &str) -> (String, String) {
    let qualified = qualified.replace('"', "");
    match qualified.split_once('.') {
        Some((schema, table)) => (schema.to_string(), table.to_string()),
        None => (String::new(), qualified),
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

fn column_items(catalog: &Catalog, schema: &str, table: &str) -> Vec<CompletionItem> {
    let Some(table) = catalog.table(schema, table) else {
        return Vec::new();
    };
    table
        .columns
        .iter()
        .map(|c| CompletionItem {
            label: c.name.clone(),
            insert: c.name.clone(),
            kind: "column".into(),
            detail: format!("{} · {}", table.name, c.data_type.to_ascii_lowercase()),
            filter: c.name.clone(),
            ..Default::default()
        })
        .collect()
}

fn table_items(catalog: &Catalog, schema: Option<&str>) -> Vec<CompletionItem> {
    catalog
        .tables
        .iter()
        .filter(|t| schema.is_none_or(|s| t.schema.eq_ignore_ascii_case(s)))
        .map(table_item)
        .collect()
}

fn table_item(t: &CatalogTable) -> CompletionItem {
    CompletionItem {
        label: t.name.clone(),
        insert: t.name.clone(),
        kind: if t.kind == TableKind::Table { "table" } else { "view" }.into(),
        detail: t.schema.clone(),
        filter: t.name.clone(),
        ..Default::default()
    }
}

const KEYWORDS: &[&str] = &[
    "select", "from", "where", "insert", "into", "values", "update", "set", "delete", "create",
    "table", "view", "index", "drop", "alter", "join", "inner", "left", "right", "full", "outer",
    "cross", "on", "as", "and", "or", "not", "null", "is", "in", "between", "like", "order",
    "group", "by", "having", "limit", "offset", "union", "all", "distinct", "case", "when",
    "then", "else", "end", "with", "primary", "key", "foreign", "references", "default",
    "constraint", "unique", "exists", "count", "sum", "avg", "min", "max", "returning",
];
