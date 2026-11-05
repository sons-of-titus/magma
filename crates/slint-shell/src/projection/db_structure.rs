//! Table structure projection: renders `TableMeta` (columns, primary key,
//! foreign keys, and indexes) into the grid model, with foreign-key cells
//! flagged so clicking `↗` or double-clicking jumps to the referenced table.

use slint::{ModelRc, VecModel};

use crate::{DbCell, DbGridColumn, DbGridRow};
use db::{ForeignKeyMeta, TableMeta};

const DEFAULTS: [(&str, f32); 4] = [
    ("kind", 90.0),
    ("name", 140.0),
    ("type / target", 180.0),
    ("attributes", 240.0),
];

pub(super) fn columns(widths: &[f32]) -> Vec<DbGridColumn> {
    DEFAULTS
        .iter()
        .enumerate()
        .map(|(i, (name, def))| DbGridColumn {
            name: (*name).into(),
            r#type: "".into(),
            width: widths.get(i).copied().unwrap_or(*def).max(40.0),
            sort: "".into(),
        })
        .collect()
}

pub(super) fn rows(detail: &TableMeta) -> Vec<DbGridRow> {
    let mut out = Vec::new();
    for (idx, col) in detail.columns.iter().enumerate() {
        let fk = detail.fk_for_column(&col.name);
        let kind_label = match (col.primary_key, fk.is_some()) {
            (true, true) => "pk · fk",
            (true, false) => "pk",
            (false, true) => "fk",
            (false, false) => "column",
        };
        let mut attrs = Vec::new();
        if col.primary_key {
            attrs.push("PRIMARY KEY".to_string());
        }
        attrs.push(if col.nullable { "NULL" } else { "NOT NULL" }.to_string());
        if col.auto_increment {
            attrs.push("AUTO INCREMENT".to_string());
        }
        if let Some(def) = &col.default {
            attrs.push(format!("DEFAULT {def}"));
        }
        if let Some((fk_meta, ref_col)) = fk {
            attrs.push(format!("→ {}.{}", qualified_ref(fk_meta), ref_col));
        }
        let kind_style = if col.primary_key {
            "pk"
        } else if fk.is_some() {
            "fk"
        } else {
            "text"
        };
        out.push(row(
            idx + 1,
            [
                cell(kind_label, kind_style, false),
                cell(&col.name, "text", fk.is_some()),
                cell(&col.data_type.to_ascii_lowercase(), "number", false),
                cell(
                    &attrs.join(" · "),
                    if fk.is_some() { "fk" } else { "text" },
                    fk.is_some(),
                ),
            ],
        ));
    }
    for fk in &detail.foreign_keys {
        let target = format!("→ {} ({})", qualified_ref(fk), fk.ref_columns.join(", "));
        let attrs = format!("({}) {target}", fk.columns.join(", "));
        let no = out.len() + 1;
        out.push(row(
            no,
            [
                cell("foreign key", "fk", false),
                cell(&fk.name, "text", true),
                cell(&target, "fk", true),
                cell(&attrs, "text", true),
            ],
        ));
    }
    for index in &detail.indexes {
        let cols = format!("({})", index.columns.join(", "));
        let kind_attr = if index.unique { "UNIQUE" } else { "INDEX" };
        let no = out.len() + 1;
        out.push(row(
            no,
            [
                cell("index", "text", false),
                cell(&index.name, "text", false),
                cell(&cols, "text", false),
                cell(kind_attr, if index.unique { "bool" } else { "text" }, false),
            ],
        ));
    }
    out
}

pub(super) fn status(detail: &TableMeta) -> (String, bool) {
    (
        format!(
            "structure · {} columns · {} foreign keys · {} indexes",
            detail.columns.len(),
            detail.foreign_keys.len(),
            detail.indexes.len()
        ),
        false,
    )
}

fn qualified_ref(fk: &ForeignKeyMeta) -> String {
    if fk.ref_schema.is_empty() || fk.ref_schema == "main" {
        fk.ref_table.clone()
    } else {
        format!("{}.{}", fk.ref_schema, fk.ref_table)
    }
}

fn cell(text: &str, kind: &str, fk: bool) -> DbCell {
    DbCell {
        text: text.into(),
        kind: kind.into(),
        edited: false,
        fk,
    }
}

fn row(no: usize, cells: [DbCell; 4]) -> DbGridRow {
    DbGridRow {
        cells: ModelRc::new(VecModel::from(Vec::from(cells))),
        status: "".into(),
        number: no.to_string().into(),
    }
}
