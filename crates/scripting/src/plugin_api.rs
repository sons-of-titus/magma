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
use super::jtable;
use super::with_editor;
use evil_janet::*;
use kernel::event::keys;
use kernel::event::payload::{
    LanguageUnregisteredPayload, PluginLoadedPayload, PluginRegisteredPayload,
};
use kernel::extension::{Capability, ExtensionManifest, PluginRecord};

use super::extension_api::{ExecContext, with_context};

// ── C functions ──────────────────────────────────────────────────────────────

/// `(plugin/register name version root entry deps langs caps)` → `:ok`
/// `(plugin/register spec-table)` → `:ok`
///
/// Register a plugin.  The positional form takes the core fields; the
/// table form is what `plugin/define` passes and carries the rest:
/// `{:name :version :root :entry :deps :languages :capabilities
///   :description :author :homepage :tags}`.  Either way the registration
/// installs the extension manifest, so the declared capabilities gate
/// this plugin's handlers at invocation time.
unsafe extern "C-unwind" fn c_plugin_register(argc: i32, argv: *mut Janet) -> Janet {
    with_editor(|ed| unsafe {
        let (name, version, root, entry, deps, langs, caps, description, author, homepage, tags) =
            if argc >= 1 && jtable::is_spec(*argv.add(0)) {
                let spec = *argv.add(0);
                let get_arr =
                    |key: &str| -> Vec<String> { jtable::get_str_array(spec, key).unwrap_or_default() };
                let get_str =
                    |key: &str| -> Option<String> { jtable::get_str(spec, key).filter(|s| !s.is_empty()) };
                (
                    get_str("name")
                        .unwrap_or_else(|| conv::signal_err("plugin/register: :name required")),
                    get_str("version").unwrap_or_else(|| "0.0.0".to_string()),
                    get_str("root").unwrap_or_default(),
                    get_str("entry").unwrap_or_else(|| "main.janet".to_string()),
                    get_arr("deps"),
                    get_arr("languages"),
                    get_arr("capabilities")
                        .iter()
                        .filter_map(|s| Capability::from_str(s))
                        .collect::<Vec<Capability>>(),
                    get_str("description"),
                    get_str("author"),
                    get_str("homepage"),
                    get_arr("tags"),
                )
            } else {
                if argc < 4 {
                    conv::signal_err("plugin/register: name version root entry required");
                }
                let name = conv::get_str(argc, argv, 0)
                    .unwrap_or_else(|| conv::signal_err("plugin/register: name must be a string"));
                (
                    name,
                    conv::get_str(argc, argv, 1).unwrap_or_else(|| "0.0.0".to_string()),
                    conv::get_str(argc, argv, 2).unwrap_or_default(),
                    conv::get_str(argc, argv, 3).unwrap_or_default(),
                    conv::get_str_array(argc, argv, 4).unwrap_or_default(),
                    conv::get_str_array(argc, argv, 5).unwrap_or_default(),
                    conv::get_str_array(argc, argv, 6)
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|s| Capability::from_str(s))
                        .collect(),
                    None,
                    None,
                    None,
                    Vec::new(),
                )
            };

        let manifest = ExtensionManifest::new(name.clone(), version.clone(), caps, entry.clone());
        ed.extension_registry.register(manifest);

        let record = PluginRecord {
            name: name.clone(),
            version: version.clone(),
            root,
            entry_point: entry,
            dependencies: deps,
            languages: langs,
            loaded: false,
            description,
            author,
            homepage,
            tags,
        };
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
            let cstr = std::ffi::CStr::from_ptr(desc as *const std::ffi::c_char);
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
/// Unload a plugin: its languages, project templates, and toolchains are
/// removed; every script-level registration its entry made (commands,
/// event subscriptions, keymap bindings — recorded in the contribution
/// journal at registration time) is removed with it; each language's
/// `language-unregistered` event fires so Janet-side per-language tables
/// clean up; and `plugin-unloaded` fires last.
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
        let owned: Vec<String> = ed
            .language_registry
            .list()
            .into_iter()
            .filter(|l| l.owner.as_deref() == Some(name.as_str()))
            .map(|l| l.name.clone())
            .collect();
        let langs_removed = ed.language_registry.remove_owner(&name);
        let templates_removed = ed.templates.remove_owner(&name);
        let toolchains_removed = ed.toolchains.remove_owner(&name);
        if let Some(record) = ed.plugin_registry.get_mut(&name) {
            record.loaded = false;
        }
        // Script-level teardown: what the entry registered, and only that.
        let mut commands = 0usize;
        let mut events = 0usize;
        let mut keymaps = 0usize;
        let mut snippets_removed_total = 0usize;
        let mut sources = 0usize;
        for contribution in ed.plugin_contributions.take(&name) {
            match contribution {
                kernel::state::contributions::Contribution::Command(cmd) => {
                    if ed.commands.unregister(&cmd) {
                        commands += 1;
                    }
                }
                kernel::state::contributions::Contribution::Event { subscription, .. } => {
                    ed.events.off(subscription);
                    super::event_api::release_handler(subscription);
                    events += 1;
                }
                kernel::state::contributions::Contribution::Keymap { layer, key } => {
                    match layer.as_deref() {
                        Some(l) => ed.keymaps.unset_layer(l, &key),
                        None => ed.keymaps.unset(&key),
                    }
                    keymaps += 1;
                }
                kernel::state::contributions::Contribution::PickerSource(id) => {
                    if ed.picker_sources.remove(&id) {
                        sources += 1;
                    }
                }
            }
        }
        // Per-language events let Janet-side registries (mode maps,
        // formatter and LSP specs) drop the language's entries.
        for lang in owned {
            let snippets_removed = ed.snippet.registry.remove_language(&lang);
            snippets_removed_total += snippets_removed;
            ed.events.emit_typed(
                keys::events::LANGUAGE_UNREGISTERED,
                LanguageUnregisteredPayload { name: lang },
            );
        }
        info!(
            "plugin unloaded: {name} ({} languages, {} templates, {} toolchains, {} commands, {} handlers, {} keybindings, {} snippets, {} picker sources)",
            langs_removed, templates_removed, toolchains_removed, commands, events, keymaps, snippets_removed_total, sources
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
