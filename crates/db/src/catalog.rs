//! Whole-schema snapshot used for completion: every table and view with its
//! columns, loaded once per connection so keystrokes never hit the wire.

use crate::{Child, DbEngine, FolderKind, NodeRef, TableKind};

/// Tables past this count keep their name but skip column detail when an
/// engine falls back to walking the tree one table at a time.
const WALK_DETAIL_CAP: usize = 400;

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub schemas: Vec<String>,
    pub tables: Vec<CatalogTable>,
}

#[derive(Debug, Clone)]
pub struct CatalogTable {
    pub schema: String,
    pub name: String,
    pub kind: TableKind,
    pub columns: Vec<CatalogColumn>,
}

#[derive(Debug, Clone)]
pub struct CatalogColumn {
    pub name: String,
    pub data_type: String,
}

impl Catalog {
    /// Case-insensitive lookup; an empty `schema` matches any schema.
    pub fn table(&self, schema: &str, name: &str) -> Option<&CatalogTable> {
        self.tables.iter().find(|t| {
            t.name.eq_ignore_ascii_case(name)
                && (schema.is_empty() || t.schema.eq_ignore_ascii_case(schema))
        })
    }

    pub fn has_schema(&self, schema: &str) -> bool {
        self.schemas.iter().any(|s| s.eq_ignore_ascii_case(schema))
    }
}

/// Engine-agnostic fallback built on the lazy tree API.
pub fn walk<E: DbEngine + ?Sized>(engine: &mut E) -> Result<Catalog, String> {
    let mut catalog = Catalog::default();
    for db in engine.introspect(&NodeRef::Root)? {
        let Child::Database { name: database, .. } = db else {
            continue;
        };
        for schema in engine.introspect(&NodeRef::Database(database.clone()))? {
            let Child::Schema { name: schema } = schema else {
                continue;
            };
            catalog.schemas.push(schema.clone());
            for folder in [FolderKind::Tables, FolderKind::Views] {
                let node = NodeRef::Folder {
                    database: database.clone(),
                    schema: schema.clone(),
                    kind: folder,
                };
                for child in engine.introspect(&node)? {
                    let Child::Table { name, kind } = child else {
                        continue;
                    };
                    let columns = if catalog.tables.len() < WALK_DETAIL_CAP {
                        let table = NodeRef::Table {
                            database: database.clone(),
                            schema: schema.clone(),
                            name: name.clone(),
                            is_view: kind != TableKind::Table,
                        };
                        engine
                            .table_detail(&table)
                            .map(|d| {
                                d.columns
                                    .into_iter()
                                    .map(|c| CatalogColumn {
                                        name: c.name,
                                        data_type: c.data_type,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    catalog.tables.push(CatalogTable {
                        schema: schema.clone(),
                        name,
                        kind,
                        columns,
                    });
                }
            }
        }
    }
    Ok(catalog)
}
