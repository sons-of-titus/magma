//! Janet API for the Plugin System.
//!
//! A plugin is an installed extension: an on-disk directory with a
//! manifest (registered here), an entry script, declared dependencies,
//! and the languages it serves.  Registering also installs the
//! extension manifest, so the capabilities a plugin declares in its
//! manifest gate every privileged call its handlers make later.
//!
//! Registered C functions:
//!   (plugin/register name version root entry deps langs caps) → :ok
//!   (plugin/load name)    → :ok or :error
//!   (plugin/list)         → array of tables
//!   (plugin/status name)  → table or nil

use super::conv;
use super::with_editor;
use evil_janet::*;
use kernel::event::keys;
use kernel::event::payload::{PluginLoadedPayload, PluginRegisteredPayload};
use kernel::extension::{Capability, ExtensionManifest, PluginRecord};

use super::extension_api::{ExecContext, with_context};

// ── C functions ──────────────────────────────────────────────────────────────

/// `(plugin/register name version root entry deps langs caps)` → `:ok`
///
/// Register a plugin.  `deps` and `langs` are arrays of names, `caps` an
/// array of capability keywords.  Registers both the plugin record and the
/// extension manifest (capabilities become enforceable for this plugin's
/// handlers).
unsafe extern "C-unwind" fn c_plugin_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        if argc < 4 {
            conv::signal_err("plugin/register: name version root entry required");
        }
        let name = conv::get_str(argc, argv, 0)
            .unwrap_or_else(|| conv::signal_err("plugin/register: name must be a string"));
        let version = conv::get_str(argc, argv, 1).unwrap_or_else(|| "0.0.0".to_string());
        let root = conv::get_str(argc, argv, 2).unwrap_or_default();
        let entry = conv::get_str(argc, argv, 3).unwrap_or_default();
        let deps = conv::get_str_array(argc, argv, 4).unwrap_or_default();
        let langs = conv::get_str_array(argc, argv, 5).unwrap_or_default();
        let caps: Vec<Capability> = conv::get_str_array(argc, argv, 6)
            .unwrap_or_default()
            .iter()
            .filter_map(|s| Capability::from_str(s))
            .collect();

        let manifest = ExtensionManifest::new(name.clone(), version.clone(), caps, entry.clone());
        ed.extension_registry.register(manifest);

        let record = PluginRecord {
            name: name.clone(),
            version,
            root,
            entry_point: entry,
            dependencies: deps,
            languages: langs,
            loaded: false,
        };
        let version = record.version.clone();
        ed.plugin_registry.register(record);
        ed.events.emit_typed(
            keys::events::PLUGIN_REGISTERED,
            PluginRegisteredPayload { name, version },
        );
        conv::keyword("ok")
    })
}

/// Evaluate `source` in the core environment, extracting the Janet error
/// value on failure — janet_dostring leaves it in the out parameter, where
/// the generic `eval_result` discards it.
unsafe fn eval_entry(source: &str) -> Result<(), String> {
    unsafe {
        let c_name = std::ffi::CString::new("plugin-entry").map_err(|e| e.to_string())?;
        let c_source = std::ffi::CString::new(source).map_err(|e| e.to_string())?;
        let mut result = std::mem::MaybeUninit::<Janet>::zeroed();
        let status = janet_dostring(
            janet_core_env(std::ptr::null_mut()),
            c_source.as_ptr(),
            c_name.as_ptr(),
            result.as_mut_ptr(),
        );
        if status == 0 {
            return Ok(());
        }
        let v = result.assume_init();
        let desc = janet_description(v);
        if desc.is_null() {
            Err("Janet error in plugin entry".to_string())
        } else {
            let cstr = std::ffi::CStr::from_ptr(desc as *const i8);
            Err(cstr.to_string_lossy().into_owned())
        }
    }
}

/// `(plugin/load name)` → `:ok` or `:error`
///
/// Evaluate the plugin's entry file with the plugin's capability context
/// active: every handler and command it registers carries those
/// capabilities at invocation time.  A `main.jimage` entry routes to
/// `plugin/load-image` inside the same context window.  Dependency
/// resolution happens before this is reached (Janet side).
unsafe extern "C-unwind" fn c_plugin_load(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let name = match unsafe { conv::get_str(argc, argv, 0) } {
            Some(s) => s,
            None => conv::signal_err("plugin/load: name string required"),
        };

        // Clone what the eval needs so no borrow of ed is held across it —
        // eval re-enters through with_editor (see language/load for the
        // established pattern).
        let (root, entry, languages) = match ed.plugin_registry.get(&name) {
            Some(p) => (p.root.clone(), p.entry_point.clone(), p.languages.clone()),
            None => return conv::keyword("error"),
        };
        let path = std::path::Path::new(&root).join(&entry);
        let bytes = match std::fs::read(&path) {
            Ok(c) => c,
            Err(e) => {
                ed.show_message(
                    kernel::state::MessageKind::Error,
                    format!("plugin {name}: cannot read {entry}: {e}"),
                );
                return conv::keyword("error");
            }
        };
        // An image is marshaled bytecode: all binary handling lives in
        // Janet; Rust sniffs only the magic and hands over the path.
        let source = if bytes.starts_with(b"MGIMG1") {
            let shown = path.display().to_string().replace('"', "\\\"");
            format!(r#"(plugin/load-image "{shown}")"#)
        } else {
            match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(e) => {
                    logging::error!("plugin_api.rs:140: {e}");
                    ed.show_message(
                        kernel::state::MessageKind::Error,
                        format!("plugin {name}: {entry} is neither source nor an image"),
                    );
                    return conv::keyword("error");
                }
            }
        };

        // janet_dostring — safe from within a C callback.
        let result = with_context(ExecContext::Extension(name.clone()), || unsafe {
            eval_entry(&source)
        });
        match result {
            Ok(()) => {
                if let Some(record) = ed.plugin_registry.get_mut(&name) {
                    record.loaded = true;
                }
                // The entry script owns the whole language surface; mark the
                // plugin's languages loaded so :lang-list reflects reality.
                for lang in &languages {
                    if ed.language_registry.get(lang).is_some() {
                        ed.language_registry.mark_loaded(lang);
                    }
                }
                ed.events
                    .emit_typed(keys::events::PLUGIN_LOADED, PluginLoadedPayload { name });
                conv::keyword("ok")
            }
            Err(e) => {
                ed.show_message(
                    kernel::state::MessageKind::Error,
                    format!("plugin {name} failed to load: {e}"),
                );
                conv::keyword("error")
            }
        }
    })
}

/// `(plugin/unload name)` → `:ok` or `:error`
///
/// Unload a plugin at the registry level: its languages, project templates,
/// and toolchains are removed, its record is marked not-loaded, and a
/// `plugin-unloaded` event fires.  Scripts the plugin already registered
/// (commands, event handlers, keymaps) keep running until the session ends
/// — the scripting runtime has no per-plugin teardown, so this is an
/// unregistration, not a sandboxed suspension.
unsafe extern "C-unwind" fn c_plugin_unload(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| {
        let Some(name) = (unsafe { conv::get_str(argc, argv, 0) }) else {
            return conv::keyword("error");
        };
        let Some(record) = ed.plugin_registry.get(&name) else {
            ed.show_message(
                kernel::state::MessageKind::Error,
                format!("plugin not registered: {name}"),
            );
            return conv::keyword("error");
        };
        if !record.loaded {
            return conv::keyword("ok");
        }
        let langs_removed = ed.language_registry.remove_owner(&name);
        let templates_removed = ed.templates.remove_owner(&name);
        let toolchains_removed = ed.toolchains.remove_owner(&name);
        if let Some(record) = ed.plugin_registry.get_mut(&name) {
            record.loaded = false;
        }
        info!(
            "plugin unloaded: {name} ({} languages, {} templates, {} toolchains)",
            langs_removed, templates_removed, toolchains_removed
        );
        ed.events.emit(
            kernel::event::keys::events::PLUGIN_UNLOADED,
            [("name".to_string(), name)].into_iter().collect(),
        );
        conv::keyword("ok")
    })
}

/// `(plugin/list)` → array of
/// `{:name :version :root :entry :deps :languages :capabilities :loaded}`
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
        let tbl = janet_wrap_table(janet_table(8));
        let t = janet_unwrap_table(tbl);
        janet_table_put(t, conv::keyword("name"), conv::string(&record.name));
        janet_table_put(t, conv::keyword("version"), conv::string(&record.version));
        janet_table_put(t, conv::keyword("root"), conv::string(&record.root));
        janet_table_put(t, conv::keyword("entry"), conv::string(&record.entry_point));

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

// ── Registration ─────────────────────────────────────────────────────────────

pub fn register() -> Vec<JanetReg> {
    vec![
        JanetReg {
            name: c"plugin/register".as_ptr() as *const _,
            cfun: Some(
                c_plugin_register as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet,
            ),
            documentation: c"Register a plugin: name version root entry deps langs capabilities"
                .as_ptr() as *const _,
        },
        JanetReg {
            name: c"plugin/load".as_ptr() as *const _,
            cfun: Some(c_plugin_load as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Evaluate a plugin's entry script under its capability context; :ok or :error"
                .as_ptr() as *const _,
        },
        JanetReg {
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
        },
        JanetReg {
            name: c"plugin/unload".as_ptr() as *const _,
            cfun: Some(c_plugin_unload as unsafe extern "C-unwind" fn(i32, *mut Janet) -> Janet),
            documentation: c"Unload a plugin: remove its languages/templates/toolchains, mark it not loaded, emit plugin-unloaded"
                .as_ptr() as *const _,
        },
    ]
}
