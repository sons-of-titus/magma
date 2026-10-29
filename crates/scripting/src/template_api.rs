//! Janet API for the project-template registry.
//!
//! Registered C functions:
//!   (template/register spec) → :ok
//!   (template/list)          → array of tables
//!   (template/get id)        → table or nil
//!
//! `template/define` in builtins/template.janet is the plugin-facing form:
//! it resolves `:source` against the declaring plugin's directory and adds
//! the plugin as owner before calling `template/register`.

use super::conv;
use super::jtable;
use super::with_editor;
use evil_janet::*;
use kernel::templates::{InputKind, ProjectTemplate, TemplateInput};

/// Parse one `{key {:type ... :required ... :default ... :choices ...}}`
/// variable entry; the table key is the variable name.
unsafe fn parse_input(key: String, spec: Janet) -> Option<TemplateInput> {
    // A non-spec variable value has no readable type; the caller already
    // reported it, but keep this total.
    unsafe {
        let type_str = jtable::get_str(spec, "type").unwrap_or_default();
        let kind = match type_str.as_str() {
            "project-name" => InputKind::ProjectName,
            "boolean" => InputKind::Boolean,
            "choice" => {
                InputKind::Choice(jtable::get_str_array(spec, "choices").unwrap_or_default())
            }
            _ => InputKind::Text,
        };
        let required = jtable::get_bool(spec, "required").unwrap_or(false);
        let default = jtable::get_str(spec, "default");
        let description = jtable::get_str(spec, "description").unwrap_or_default();
        Some(TemplateInput {
            key,
            kind,
            required,
            default,
            description,
        })
    }
}

/// `(template/register spec)` → `:ok`
///
/// Register a project template from a spec table:
/// `{:id "rust.binary" :name ... :description ... :category ...
///   :language "rust" :source "/abs/dir" :variables {...}
///   :toolchain "rust.cargo" :debug-adapter "codelldb"}`
///
/// The source directory must be absolute and exist.  Registering the same
/// id from the same plugin replaces the entry; another plugin claiming the
/// id is an error.
unsafe extern "C-unwind" fn c_template_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 1 {
            conv::signal_err("template/register: spec table required");
        }
        let spec = *argv.add(0);
        if !jtable::is_spec(spec) {
            conv::signal_err("template/register: spec must be a table or struct");
        };
        let Some(id) = jtable::get_str(spec, "id") else {
            conv::signal_err("template/register: :id is required");
        };
        let Some(name) = jtable::get_str(spec, "name") else {
            conv::signal_err("template/register: :name is required");
        };
        let Some(source) = jtable::get_str(spec, "source") else {
            conv::signal_err("template/register: :source is required");
        };
        let mut variables = Vec::new();
        if let Some(vars_spec) = jtable::get_spec(spec, "variables") {
            for (key, input_spec) in jtable::string_spec_entries(vars_spec) {
                let Some(input) = parse_input(key.clone(), input_spec) else {
                    conv::signal_err(&format!(
                        "template/register: variable {key:?} has an invalid spec"
                    ));
                };
                variables.push(input);
            }
        }
        let owner = jtable::get_str(spec, "plugin").unwrap_or_default();

        let template = ProjectTemplate {
            id,
            name,
            description: jtable::get_str(spec, "description").unwrap_or_default(),
            category: jtable::get_str(spec, "category").unwrap_or_else(|| "other".into()),
            language: jtable::get_str(spec, "language").unwrap_or_default(),
            source: source.into(),
            variables,
            toolchain: jtable::get_str(spec, "toolchain"),
            file_count: 0,
            owner,
        };
        match ed.templates.register(template) {
            Ok(()) => conv::keyword("ok"),
            Err(e) => conv::signal_err(&e),
        }
    })
}

/// Template metadata as a Janet table (the wizard consumes the Rust side
/// directly; this table is for scripts and tests).
unsafe fn template_table(t: &kernel::templates::ProjectTemplate) -> Janet {
    unsafe {
        let tbl = janet_wrap_table(janet_table(12));
        let t_tbl = janet_unwrap_table(tbl);
        janet_table_put(t_tbl, conv::keyword("id"), conv::string(&t.id));
        janet_table_put(t_tbl, conv::keyword("name"), conv::string(&t.name));
        janet_table_put(
            t_tbl,
            conv::keyword("description"),
            conv::string(&t.description),
        );
        janet_table_put(t_tbl, conv::keyword("category"), conv::string(&t.category));
        janet_table_put(t_tbl, conv::keyword("language"), conv::string(&t.language));
        janet_table_put(
            t_tbl,
            conv::keyword("source"),
            conv::string(&t.source.to_string_lossy()),
        );
        janet_table_put(
            t_tbl,
            conv::keyword("toolchain"),
            opt_str(t.toolchain.as_deref()),
        );
        janet_table_put(
            t_tbl,
            conv::keyword("file-count"),
            conv::integer(t.file_count as i32),
        );
        janet_table_put(t_tbl, conv::keyword("plugin"), conv::string(&t.owner));
        let vars = janet_array(t.variables.len().max(1) as i32);
        for input in &t.variables {
            let vt = janet_wrap_table(janet_table(5));
            let v_tbl = janet_unwrap_table(vt);
            janet_table_put(v_tbl, conv::keyword("key"), conv::string(&input.key));
            let type_kw = match &input.kind {
                InputKind::Text => "text",
                InputKind::ProjectName => "project-name",
                InputKind::Boolean => "boolean",
                InputKind::Choice(_) => "choice",
            };
            janet_table_put(v_tbl, conv::keyword("type"), conv::string(type_kw));
            janet_table_put(
                v_tbl,
                conv::keyword("required"),
                conv::boolean(input.required),
            );
            janet_table_put(
                v_tbl,
                conv::keyword("default"),
                opt_str(input.default.as_deref()),
            );
            if let InputKind::Choice(choices) = &input.kind {
                let arr = janet_array(choices.len().max(1) as i32);
                for c in choices {
                    janet_array_push(arr, conv::string(c));
                }
                janet_table_put(v_tbl, conv::keyword("choices"), janet_wrap_array(arr));
            }
            janet_array_push(vars, vt);
        }
        janet_table_put(t_tbl, conv::keyword("variables"), janet_wrap_array(vars));
        tbl
    }
}

unsafe fn opt_str(s: Option<&str>) -> Janet {
    match s {
        Some(s) => conv::string(s),
        None => conv::nil(),
    }
}

/// `(template/list)` → array of template tables, sorted by (category, id)
unsafe extern "C-unwind" fn c_template_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let templates = ed.templates.list();
        let arr = janet_wrap_array(janet_array(templates.len().max(1) as i32));
        for t in templates {
            janet_array_push(janet_unwrap_array(arr), template_table(t));
        }
        arr
    })
}

/// `(template/get id)` → table or nil
unsafe extern "C-unwind" fn c_template_get(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let Some(id) = conv::get_str(argc, argv, 0) else {
            return conv::nil();
        };
        match ed.templates.get(&id) {
            Some(t) => template_table(t),
            None => conv::nil(),
        }
    })
}

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"template/register".as_ptr() as *const _,
            cfun: Some(c_template_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Register a project template from a spec table; see template/define for the plugin-facing form".as_ptr() as *const _,
        },
        JanetReg {
            name: c"template/list".as_ptr() as *const _,
            cfun: Some(c_template_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return an array of {:id :name :description :category :language :source :variables :toolchain :file-count :plugin} tables sorted by category and id".as_ptr() as *const _,
        },
        JanetReg {
            name: c"template/get".as_ptr() as *const _,
            cfun: Some(c_template_get as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return one template's metadata table by id, or nil".as_ptr() as *const _,
        },
    ]
}
