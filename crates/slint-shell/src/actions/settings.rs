//! Settings and dialog wiring.  Every change is applied immediately through
//! the same paths the egui shell uses — `Editor.options` for registry
//! options, `Editor.font_config` for fonts — so there is no staged local
//! state and closing the window never discards anything.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use kernel::command::execute_command;
use kernel::state::persist;
use slint::ComponentHandle;

use super::fonts::{FontChanges, apply_fonts, collect_available_fonts};
use crate::{AppState, AppWindow, sync::refresh};

// Settings information architecture.  Categories beyond the option
// registry's own are the Agent page (provider, key, model) and Advanced
// (raw custom options).
pub(super) const CATEGORY_APPEARANCE: i32 = 0;
pub(super) const CATEGORY_AGENT: i32 = 3;

pub(super) fn install(app: &AppWindow, state: Rc<RefCell<AppState>>) {
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_apply_font_settings(move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let app_font_name = app.get_settings_app_font_name().to_string();
            let app_font_size: f32 = app.get_settings_app_font_size() as f32;
            let editor_font_name = app.get_settings_editor_font_name().to_string();
            let editor_font_size: f32 = app.get_settings_editor_font_size() as f32;
            let ligatures = app.get_settings_ligatures();

            let mut s = state.borrow_mut();
            apply_fonts(
                &weak,
                &mut s,
                FontChanges {
                    app_font_name,
                    app_font_size,
                    editor_font_name,
                    editor_font_size,
                    ligatures,
                },
            );
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_apply_agent_settings(move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let provider = app.get_settings_agent_provider().to_string();
            let token = app.get_settings_agent_token().to_string();
            let model = app.get_settings_agent_model().to_string();
            let mut s = state.borrow_mut();
            let mut changed = false;
            if !provider.trim().is_empty() {
                s.editor
                    .options
                    .insert("agent.provider".into(), provider.clone());
                changed = true;
            }
            if !token.trim().is_empty() {
                s.editor
                    .options
                    .insert(format!("agent.api-key.{}", provider), token);
                changed = true;
            }
            if !model.trim().is_empty() {
                s.editor.options.insert("agent.model".into(), model);
                changed = true;
            }
            if changed {
                persist::save_settings(&s.editor);
            }
            s.message =
                if changed && kernel::agent::llm::LlmConfig::is_configured(&s.editor.options) {
                    "agent settings saved".into()
                } else if changed {
                    "agent settings saved — still missing an API key".into()
                } else {
                    "nothing to save".into()
                };
            s.editor.frame_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_set_option(move |name, value| {
            let mut s = state.borrow_mut();
            s.editor.options.insert(name.to_string(), value.to_string());
            persist::save_settings(&s.editor);
            // Chrome-relevant options need the shell to re-read them.
            if name == "gui-theme" {
                s.theme_name.clear();
            }
            s.settings_rows_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_reset_option(move |name| {
            let mut s = state.borrow_mut();
            if let Some(spec) = kernel::state::options::OPTION_SPECS
                .iter()
                .find(|sp| sp.name == name.as_str())
            {
                s.editor
                    .options
                    .insert(spec.name.to_string(), spec.default.to_string());
                if spec.name == "gui-theme" {
                    s.theme_name.clear();
                }
            } else {
                s.editor.options.remove(&name.to_string());
            }
            persist::save_settings(&s.editor);
            s.settings_rows_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_add_option(move |name, value| {
            let name = name.trim().to_string();
            if name.is_empty() {
                return;
            }
            let mut s = state.borrow_mut();
            s.editor.options.insert(name, value.to_string());
            persist::save_settings(&s.editor);
            s.settings_rows_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_settings_search_changed(move || {
            let mut s = state.borrow_mut();
            if let Some(a) = weak.upgrade() {
                s.settings_search = a.get_settings_search().to_string();
            }
            s.settings_rows_dirty = true;
            refresh(&weak, &mut s);
        });
    }
    {
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        app.on_settings_select_category(move |category| {
            let category = category.max(0).min(4);
            // The page branches on the view's property, and refresh reads the
            // category back from it — state alone would be clobbered before
            // the rows were ever projected.
            if let Some(a) = weak.upgrade() {
                a.set_settings_category(category);
            }
            let mut s = state.borrow_mut();
            s.settings_category = category;
            s.settings_rows_dirty = true;
            refresh(&weak, &mut s);
        });
    }
}

/// Show the settings window on the given category, re-reading every field
/// from live editor state.
pub(super) fn open(weak: &slint::Weak<AppWindow>, s: &mut AppState, category: i32) {
    let Some(a) = weak.upgrade() else { return };
    a.set_settings_visible(true);
    a.set_settings_category(category);
    s.settings_rows_dirty = true;
    a.set_settings_app_font_name(s.app_font_name.clone().into());
    a.set_settings_app_font_size(s.app_font_size as i32);
    a.set_settings_editor_font_name(s.editor.font_config.family.clone().into());
    a.set_settings_editor_font_size(s.editor.font_config.size as i32);
    a.set_settings_ligatures(s.editor.font_config.ligatures);
    let prov = s
        .editor
        .options
        .get("agent.provider")
        .cloned()
        .unwrap_or_else(|| "openai".into());
    a.set_settings_agent_provider(prov.clone().into());
    let token = s
        .editor
        .options
        .get(&format!("agent.api-key.{}", prov))
        .cloned()
        .or_else(|| s.editor.options.get("agent.api-key").cloned())
        .unwrap_or_default();
    a.set_settings_agent_token(token.into());
    let model = s
        .editor
        .options
        .get("agent.model")
        .cloned()
        .unwrap_or_default();
    a.set_settings_agent_model(model.into());
    let fonts: Vec<slint::SharedString> = collect_available_fonts(&s.editor, &s.app_font_name)
        .into_iter()
        .map(Into::into)
        .collect();
    a.set_available_fonts(slint::ModelRc::new(slint::VecModel::from(fonts)));
}

/// `command()` cases routed from the shared dispatcher.
pub(super) fn command(weak: &slint::Weak<AppWindow>, s: &mut AppState, command: &str) {
    match command {
        "dialog:settings" => {
            let category = match s.settings_category {
                0..=4 => s.settings_category,
                _ => CATEGORY_APPEARANCE,
            };
            open(weak, s, category);
        }
        "dialog:find" | "dialog:goto-file" | "dialog:goto-symbol" => {
            s.dialog_title = match command {
                "dialog:find" => "Find in Buffers",
                "dialog:goto-file" => "Go to File",
                _ => "Go to Symbol",
            }
            .into();
            s.dialog_input.clear();
            s.dialog_visible = true;
            if let Some(a) = weak.upgrade() {
                a.set_dialog_input("".into());
            }
        }
        "dialog:ok" => {
            let cmd = weak
                .upgrade()
                .map(|a| a.get_dialog_input().to_string())
                .unwrap_or_default();
            if !cmd.trim().is_empty() {
                match execute_command(&mut s.editor, &cmd, &HashMap::new()) {
                    Ok(()) => s.message.clear(),
                    Err(e) => s.message = e,
                }
            }
            s.dialog_visible = false;
            s.dialog_input.clear();
            if let Some(a) = weak.upgrade() {
                a.set_dialog_input("".into());
            }
        }
        c if c.starts_with("settings:theme:") => {
            let name = c.trim_start_matches("settings:theme:");
            s.editor.options.insert("gui-theme".into(), name.into());
            persist::save_settings(&s.editor);
            s.theme_name.clear();
        }
        _ => {}
    }
}
