//! Data-editor change tracking and statement generation.  The grid records
//! cell edits, row inserts, and row deletes against the fetched page; Apply
//! turns them into one transaction on the engine, with optimistic
//! concurrency predicates so a row changed underneath fails loudly.

use crate::{ColumnMeta, Dialect, TableMeta};

/// Identifies the table being edited; the engine quotes it per dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRef {
    pub database: String,
    pub schema: String,
    pub name: String,
}

/// One pending change.  Row/column indexes address the fetched page;
/// `original_row` is the page snapshot the edit was made against, so Apply
/// can detect a concurrent modification.
#[derive(Debug, Clone, PartialEq)]
pub enum EditOp {
    UpdateCell {
        row: usize,
        col: usize,
        value: crate::DbValue,
        original_row: Vec<crate::DbValue>,
    },
    DeleteRow {
        row: usize,
        original_row: Vec<crate::DbValue>,
    },
    /// A new row's cells, aligned to the table's columns.
    InsertRow {
        values: Vec<crate::DbValue>,
    },
}

/// Everything a tab accumulates before Apply.
#[derive(Debug, Clone, Default)]
pub struct PendingEdits {
    pub ops: Vec<EditOp>,
}

impl PendingEdits {
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn update_count(&self) -> usize {
        self.ops.iter().filter(|o| matches!(o, EditOp::UpdateCell { .. })).count()
    }

    pub fn delete_count(&self) -> usize {
        self.ops.iter().filter(|o| matches!(o, EditOp::DeleteRow { .. })).count()
    }

    pub fn insert_count(&self) -> usize {
        self.ops.iter().filter(|o| matches!(o, EditOp::InsertRow { .. })).count()
    }

    /// Drop every pending change on one row (revert row).
    pub fn revert_row(&mut self, row: usize) {
        self.ops.retain(|op| match op {
            EditOp::UpdateCell { row: r, .. } | EditOp::DeleteRow { row: r, .. } => *r != row,
            EditOp::InsertRow { .. } => true,
        });
    }

    pub fn clear(&mut self) {
        self.ops.clear();
    }

    /// The generated statements, one per op, in order.  Also the exact text
    /// the "inspect generated SQL" affordance shows.
    pub fn statements(&self, table: &TableMeta, dialect: Dialect) -> Result<Vec<String>, String> {
        generate_ops(
            &self.ops,
            table,
            &TableRef {
                database: table.database.clone(),
                schema: table.schema.clone(),
                name: table.name.clone(),
            },
            dialect,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct EditResult {
    /// Statements executed before the result (all of them on success).
    pub statements_run: usize,
    pub rows_affected: i64,
}

/// Quote the table reference per dialect.  SQL Server uses three-part
/// names, PostgreSQL two-part, MySQL/MariaDB database-qualified, SQLite
/// bare — the database is implicit in the connection.
pub fn quote_table(t: &TableRef, dialect: Dialect) -> String {
    let q = |s: &str| dialect.quote_ident(s);
    match dialect {
        Dialect::SqlServer => format!("{}.{}.{}", q(&t.database), q(&t.schema), q(&t.name)),
        Dialect::Postgres | Dialect::Ansi => format!("{}.{}", q(&t.schema), q(&t.name)),
        Dialect::MySql => format!("{}.{}", q(&t.database), q(&t.name)),
        Dialect::Sqlite => q(&t.name),
    }
}

/// Build the statements for the recorded ops against `table`.  Without a
/// discoverable primary key, updates and deletes cannot target rows safely
/// and the caller must refuse them.
pub fn generate_ops(
    ops: &[EditOp],
    table: &TableMeta,
    target: &TableRef,
    dialect: Dialect,
) -> Result<Vec<String>, String> {
    let pk = table
        .pk_columns()
        .ok_or_else(|| "table has no primary key; rows cannot be edited safely".to_string())?;
    let q = |s: &str| dialect.quote_ident(s);
    let target = quote_table(target, dialect);
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        match op {
            EditOp::UpdateCell { row: _, col, value, original_row } => {
                let column = table.columns.get(*col).ok_or("column out of range")?;
                let mut predicate: Vec<String> = pk
                    .iter()
                    .map(|&ci| {
                        let orig = original_row.get(ci).cloned().unwrap_or(crate::DbValue::Null);
                        dialect.null_safe_eq(&table.columns[ci].name, &dialect.literal(&orig))
                    })
                    .collect();
                // Optimistic concurrency: the edited cell's original value
                // must still match, or someone changed the row underneath.
                if !pk.contains(col) {
                    let orig = original_row.get(*col).cloned().unwrap_or(crate::DbValue::Null);
                    predicate.push(dialect.null_safe_eq(&column.name, &dialect.literal(&orig)));
                }
                out.push(format!(
                    "UPDATE {target} SET {} = {} WHERE {}",
                    q(&column.name),
                    dialect.literal(value),
                    predicate.join(" AND "),
                ));
            }
            EditOp::DeleteRow { original_row, .. } => {
                let predicate: Vec<String> = pk
                    .iter()
                    .map(|&ci| {
                        let orig = original_row.get(ci).cloned().unwrap_or(crate::DbValue::Null);
                        dialect.null_safe_eq(&table.columns[ci].name, &dialect.literal(&orig))
                    })
                    .collect();
                out.push(format!("DELETE FROM {target} WHERE {}", predicate.join(" AND ")));
            }
            EditOp::InsertRow { values } => {
                let mut cols = Vec::new();
                let mut lits = Vec::new();
                for (ci, col) in table.columns.iter().enumerate() {
                    let v = values.get(ci).cloned().unwrap_or(crate::DbValue::Null);
                    if v.is_null() && col.auto_increment {
                        continue;
                    }
                    cols.push(q(&col.name));
                    lits.push(dialect.literal(&v));
                }
                if cols.is_empty() {
                    out.push(format!("INSERT INTO {target} DEFAULT VALUES"));
                } else {
                    out.push(format!(
                        "INSERT INTO {target} ({}) VALUES ({})",
                        cols.join(", "),
                        lits.join(", "),
                    ));
                }
            }
        }
    }
    Ok(out)
}

/// Column metadata for a fresh insert row: which columns the grid seeds.
pub fn insert_template(table: &TableMeta) -> Vec<(usize, ColumnMeta)> {
    table
        .columns
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.auto_increment)
        .map(|(i, c)| (i, c.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TableKind};

    fn table() -> TableMeta {
        TableMeta {
            database: "db".into(),
            schema: "public".into(),
            name: "users".into(),
            kind: TableKind::Table,
            columns: vec![
                ColumnMeta {
                    name: "id".into(),
                    data_type: "integer".into(),
                    nullable: false,
                    default: None,
                    primary_key: true,
                    auto_increment: true,
                },
                ColumnMeta {
                    name: "name".into(),
                    data_type: "text".into(),
                    nullable: false,
                    default: None,
                    primary_key: false,
                    auto_increment: false,
                },
                ColumnMeta {
                    name: "score".into(),
                    data_type: "numeric".into(),
                    nullable: true,
                    default: None,
                    primary_key: false,
                    auto_increment: false,
                },
            ],
            primary_key: vec!["id".into()],
            foreign_keys: vec![],
            indexes: vec![],
        }
    }

    #[test]
    fn update_statement_targets_pk_and_original_value() {
        let t = table();
        let edits = PendingEdits {
            ops: vec![EditOp::UpdateCell {
                row: 0,
                col: 1,
                value: crate::DbValue::Text("ada".into()),
                original_row: vec![crate::DbValue::Integer(1), crate::DbValue::Text("grace".into()), crate::DbValue::Null],
            }],
        };
        let sql = edits.statements(&t, Dialect::Postgres).unwrap();
        assert_eq!(
            sql,
            vec![
                "UPDATE \"public\".\"users\" SET \"name\" = 'ada' WHERE \"id\" IS NOT DISTINCT FROM 1 AND \"name\" IS NOT DISTINCT FROM 'grace'"
                    .to_string()
            ]
        );
    }

    #[test]
    fn delete_statement_includes_every_pk_column() {
        let t = table();
        let edits = PendingEdits {
            ops: vec![EditOp::DeleteRow {
                row: 0,
                original_row: vec![crate::DbValue::Integer(7)],
            }],
        };
        let sql = edits.statements(&t, Dialect::Postgres).unwrap();
        assert_eq!(
            sql,
            vec!["DELETE FROM \"public\".\"users\" WHERE \"id\" IS NOT DISTINCT FROM 7".to_string()]
        );
    }

    #[test]
    fn insert_skips_auto_increment_and_defaults_nulls() {
        let t = table();
        let edits = PendingEdits {
            ops: vec![EditOp::InsertRow {
                values: vec![
                    crate::DbValue::Null,
                    crate::DbValue::Text("new".into()),
                    crate::DbValue::Integer(5),
                ],
            }],
        };
        let sql = edits.statements(&t, Dialect::MySql).unwrap();
        assert_eq!(
            sql,
            vec![
                "INSERT INTO `db`.`users` (`name`, `score`) VALUES ('new', 5)".to_string()
            ]
        );
    }

    #[test]
    fn no_primary_key_refuses_edits() {
        let mut t = table();
        t.primary_key.clear();
        let edits = PendingEdits {
            ops: vec![EditOp::DeleteRow { row: 0, original_row: vec![] }],
        };
        assert!(edits.statements(&t, Dialect::Postgres).is_err());
    }

    #[test]
    fn sqlite_quotes_and_uses_is_for_nulls() {
        let t = table();
        let edits = PendingEdits {
            ops: vec![EditOp::DeleteRow {
                row: 0,
                original_row: vec![crate::DbValue::Null],
            }],
        };
        let sql = edits.statements(&t, Dialect::Sqlite).unwrap();
        assert_eq!(sql[0], "DELETE FROM \"users\" WHERE \"id\" IS NULL");
    }

    #[test]
    fn sqlserver_uses_brackets_and_three_part_names() {
        let t = table();
        let edits = PendingEdits {
            ops: vec![EditOp::DeleteRow {
                row: 0,
                original_row: vec![crate::DbValue::Integer(1)],
            }],
        };
        let sql = edits.statements(&t, Dialect::SqlServer).unwrap();
        assert_eq!(
            sql[0],
            "DELETE FROM [db].[public].[users] WHERE [id] = 1"
        );
    }

    #[test]
    fn revert_row_drops_only_that_rows_ops() {
        let mut edits = PendingEdits {
            ops: vec![
                EditOp::UpdateCell {
                    row: 1,
                    col: 1,
                    value: crate::DbValue::Null,
                    original_row: vec![],
                },
                EditOp::DeleteRow { row: 2, original_row: vec![] },
                EditOp::InsertRow { values: vec![] },
            ],
        };
        edits.revert_row(1);
        assert_eq!(edits.ops.len(), 2);
        assert!(matches!(edits.ops[0], EditOp::DeleteRow { row: 2, .. }));
    }

    #[test]
    fn text_literals_escape_single_quotes() {
        let t = table();
        let edits = PendingEdits {
            ops: vec![EditOp::InsertRow {
                values: vec![
                    crate::DbValue::Null,
                    crate::DbValue::Text("o'brien; drop".into()),
                    crate::DbValue::Null,
                ],
            }],
        };
        let sql = edits.statements(&t, Dialect::Postgres).unwrap();
        assert!(sql[0].contains("'o''brien; drop'"));
    }

    #[test]
    fn insert_template_skips_auto_increment() {
        let t = table();
        let cols = insert_template(&t);
        assert_eq!(cols.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![1, 2]);
    }
}
