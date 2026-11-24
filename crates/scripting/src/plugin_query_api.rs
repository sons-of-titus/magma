//! Plugin query surface — `(plugin/list)` and `(plugin/status)`.
//!
//! Split from plugin_api.rs (lifecycle) so each file stays small; the
//! record table both read.

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::extension::PluginRecord;

unsafe extern "C-unwind" fn c_plugin_list(_argc: i32, _argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let records: Vec<PluginRecord> = ed.plugin_registry.list().into_iter().cloned().collect();
        let arr = janet_wrap_array(janet_array(records.len() as i32));
        for record in &records {
            let tbl = plugin_record_table(ed, record);
            janet_array_push(janet_unwrap_array(arr), tbl);
        }
        arr
    })
}

/// `(plugin/status name)` → table or nil
unsafe extern "C-unwind" fn c_plugin_status(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let name = match conv::get_str(argc, argv, 0) {
            Some(s) => s,
            None => return conv::nil(),
        };
        match ed.plugin_registry.get(&name).cloned() {
            None => conv::nil(),
            Some(record) => plugin_record_table(ed, &record),
        }
    })
}

unsafe fn plugin_record_table(ed: &mut kernel::state::Editor, record: &PluginRecord) -> Janet {
    unsafe {
        let tbl = janet_wrap_table(janet_table(12));
        let t = janet_unwrap_table(tbl);
        janet_table_put(t, conv::keyword("name"), conv::string(&record.name));
        janet_table_put(t, conv::keyword("version"), conv::string(&record.version));
        janet_table_put(t, conv::keyword("root"), conv::string(&record.root));
        janet_table_put(t, conv::keyword("entry"), conv::string(&record.entry_point));
        put_opt(t, "description", record.description.as_deref());
        put_opt(t, "author", record.author.as_deref());
        put_opt(t, "homepage", record.homepage.as_deref());

        let deps = janet_wrap_array(janet_array(record.dependencies.len() as i32));
        for dep in &record.dependencies {
            janet_array_push(janet_unwrap_array(deps), conv::string(dep));
        }
        janet_table_put(t, conv::keyword("deps"), deps);

        let langs = janet_wrap_array(janet_array(record.languages.len() as i32));
        for lang in &record.languages {
            janet_array_push(janet_unwrap_array(langs), conv::string(lang));
        }
        janet_table_put(t, conv::keyword("languages"), langs);

        let tags = janet_wrap_array(janet_array(record.tags.len() as i32));
        for tag in &record.tags {
            janet_array_push(janet_unwrap_array(tags), conv::string(tag));
        }
        janet_table_put(t, conv::keyword("tags"), tags);

        let caps = janet_wrap_array(janet_array(4));
        if let Some(manifest) = ed.extension_registry.get(&record.name) {
            for cap in &manifest.capabilities {
                janet_array_push(janet_unwrap_array(caps), conv::keyword(cap.as_str()));
            }
        }
        janet_table_put(t, conv::keyword("capabilities"), caps);

        janet_table_put(t, conv::keyword("loaded"), conv::boolean(record.loaded));
        tbl
    }
}

unsafe fn put_opt(t: *mut JanetTable, key: &str, value: Option<&str>) {
    unsafe {
        let v = match value {
            Some(s) => conv::string(s),
            None => conv::nil(),
        };
        janet_table_put(t, conv::keyword(key), v);
    }
}

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![JanetReg {
            name: c"plugin/list".as_ptr() as *const _,
            cfun: Some(c_plugin_list as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Return all registered plugins as an array of tables".as_ptr()
                as *const _,
        },
        JanetReg {
            name: c"plugin/status".as_ptr() as *const _,
            cfun: Some(
                c_plugin_status as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Return a plugin's status table, or nil if not registered".as_ptr()
                as *const _,
        }]
}
