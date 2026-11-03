//! Page-query construction for the table data editor.  The filter text is
//! a raw SQL fragment by design — the filter bar is a power tool.

use crate::{Dialect, TableMeta, edit::quote_table, TableRef};

/// The SELECT behind one data-editor page: `SELECT * FROM t [WHERE f]
/// [ORDER BY c dir] LIMIT n OFFSET p*n`.
pub fn page_query(
    dialect: Dialect,
    table: &TableMeta,
    sort: Option<(usize, bool)>,
    filter: &str,
    page: usize,
    limit: usize,
) -> String {
    let target = quote_table(
        &TableRef {
            database: table.database.clone(),
            schema: table.schema.clone(),
            name: table.name.clone(),
        },
        dialect,
    );
    let mut sql = format!("SELECT * FROM {target}");
    let filter = filter.trim();
    if !filter.is_empty() {
        sql.push_str(&format!(" WHERE ({filter})"));
    }
    if let Some((col, asc)) = sort
        && let Some(column) = table.columns.get(col)
    {
        sql.push_str(&format!(
            " ORDER BY {} {}",
            dialect.quote_ident(&column.name),
            if asc { "ASC" } else { "DESC" }
        ));
    }
    sql.push_str(&format!(" LIMIT {limit} OFFSET {}", page * limit));
    sql
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TableKind;

    fn table() -> TableMeta {
        TableMeta {
            database: "db".into(),
            schema: "public".into(),
            name: "users".into(),
            kind: TableKind::Table,
            columns: vec![crate::ColumnMeta {
                name: "id".into(),
                data_type: "integer".into(),
                nullable: false,
                default: None,
                primary_key: true,
                auto_increment: true,
            }],
            primary_key: vec!["id".into()],
            foreign_keys: vec![],
            indexes: vec![],
        }
    }

    #[test]
    fn builds_pagination_with_sort_and_filter() {
        let t = table();
        let sql = page_query(Dialect::Postgres, &t, Some((0, false)), "id > 3", 2, 200);
        assert_eq!(
            sql,
            "SELECT * FROM \"public\".\"users\" WHERE (id > 3) ORDER BY \"id\" DESC LIMIT 200 OFFSET 400"
        );
    }

    #[test]
    fn bare_page_has_no_clauses() {
        let t = table();
        let sql = page_query(Dialect::MySql, &t, None, "  ", 0, 100);
        assert_eq!(sql, "SELECT * FROM `db`.`users` LIMIT 100 OFFSET 0");
    }

    #[test]
    fn invalid_sort_column_is_ignored() {
        let t = table();
        let sql = page_query(Dialect::Sqlite, &t, Some((9, true)), "", 0, 50);
        assert_eq!(sql, "SELECT * FROM \"users\" LIMIT 50 OFFSET 0");
    }
}
