//! Postgres bulk catalog: one round trip for every table, view and column
//! outside the system schemas.

use tokio_postgres::Client;

use crate::postgres::err;
use crate::{Catalog, CatalogColumn, CatalogTable, TableKind};

pub(crate) async fn load(client: &Client) -> Result<Catalog, String> {
    let schemas = client
        .query(
            "SELECT nspname FROM pg_namespace \
             WHERE nspname NOT LIKE 'pg\\_%' AND nspname <> 'information_schema' \
             ORDER BY nspname",
            &[],
        )
        .await
        .map_err(err)?
        .iter()
        .map(|r| r.try_get::<_, String>(0).map_err(err))
        .collect::<Result<Vec<_>, String>>()?;

    let rows = client
        .query(
            "SELECT n.nspname, c.relname, c.relkind::text, a.attname, \
                    format_type(a.atttypid, a.atttypmod) \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             LEFT JOIN pg_attribute a \
                    ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
             WHERE c.relkind IN ('r','p','v','m','f') \
               AND n.nspname NOT LIKE 'pg\\_%' AND n.nspname <> 'information_schema' \
             ORDER BY n.nspname, c.relname, a.attnum",
            &[],
        )
        .await
        .map_err(err)?;

    let mut tables: Vec<CatalogTable> = Vec::new();
    for row in &rows {
        let schema: String = row.try_get(0).map_err(err)?;
        let name: String = row.try_get(1).map_err(err)?;
        let relkind: String = row.try_get(2).map_err(err)?;
        let column: Option<String> = row.try_get(3).map_err(err)?;
        let data_type: Option<String> = row.try_get(4).map_err(err)?;
        let starts_new_table = tables
            .last()
            .is_none_or(|t| t.schema != schema || t.name != name);
        if starts_new_table {
            tables.push(CatalogTable {
                schema,
                name,
                kind: match relkind.as_str() {
                    "v" => TableKind::View,
                    "m" => TableKind::MaterializedView,
                    _ => TableKind::Table,
                },
                columns: Vec::new(),
            });
        }
        if let (Some(name), Some(data_type), Some(table)) = (column, data_type, tables.last_mut()) {
            table.columns.push(CatalogColumn { name, data_type });
        }
    }
    Ok(Catalog { schemas, tables })
}
