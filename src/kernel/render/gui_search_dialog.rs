//! Search dialog — Phase 8.
//!
//! In-buffer search with smart-case matching, a results list, and
//! replace-all in the focused buffer.  Like every dialog it returns
//! `DialogAction`s for the GUI shell to apply rather than mutating the
//! editor directly.

use eframe::egui::{self, Color32, FontFamily, FontId, RichText};
use crate::kernel::state::Editor;
use super::gui_dialog::DialogAction;

// ── SearchDialog ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SearchHit {
    pub buf_id:  usize,
    pub buf_name: String,
    pub line:    usize, // 1-indexed
    pub text:    String,
}

pub struct SearchDialog {
    pub query:     String,
    pub replace:   String,
    /// `.*` mode: the query compiles as a regular expression.
    pub regex:     bool,
    pub(crate) results:    Vec<SearchHit>,
    pub(crate) last_query: String,
    /// Index of the hit Enter will visit next (Enter = next, Shift-Enter =
    /// previous — vim's n/N without stealing keys from the text field).
    pub(crate) selected: usize,
}

const SEARCH_CAP: usize = 200;

impl SearchDialog {
    pub fn new() -> Self {
        Self { query: String::new(), replace: String::new(), regex: false, results: Vec::new(), last_query: String::new(), selected: 0 }
    }

    /// Scan open buffers for `query`, capped at `SEARCH_CAP` hits.
    pub fn search(&mut self, editor: &Editor) {
        self.results.clear();
        if self.query.is_empty() { return; }
        let Ok(query) = crate::kernel::command::builtin::search_query::SearchQuery::compile(
            &self.query, self.regex) else { return };
        'outer: for (buf_id, arc) in editor.buffers.iter() {
            let buf = arc.lock().unwrap_or_else(|e| e.into_inner());
            let name = buf.name.clone();
            let text = buf.slice(0, buf.len());
            for (line_idx, line) in text.lines().enumerate() {
                let hit = match &query {
                    crate::kernel::command::builtin::search_query::SearchQuery::Literal(n)
                        => line.contains(n.as_str()),
                    crate::kernel::command::builtin::search_query::SearchQuery::Regex(re)
                        => re.is_match(line),
                };
                if hit {
                    self.results.push(SearchHit {
                        buf_id,
                        buf_name: name.clone(),
                        line: line_idx + 1,
                        text: line.trim_end().to_string(),
                    });
                    if self.results.len() >= SEARCH_CAP { break 'outer; }
                }
            }
        }
    }

    #[cfg(test)]
    pub fn results(&self) -> &[SearchHit] {
        &self.results
    }

    /// Returns actions + whether the dialog should close.
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, editor: &Editor, actions: &mut Vec<DialogAction>) -> bool {
        let query_resp = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Find in open buffers…")
                .font(FontId::new(13.0, FontFamily::Monospace))
                .desired_width(f32::INFINITY),
        );
        if self.query != self.last_query {
            self.last_query = self.query.clone();
            self.search(editor);
        }
        ui.checkbox(&mut self.regex, ".*")
            .on_hover_text("regular expression (smart case applies to literal queries)");
        ui.add(
            egui::TextEdit::singleline(&mut self.replace)
                .hint_text("Replace with…")
                .font(FontId::new(13.0, FontFamily::Monospace))
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button("Replace next").clicked() && !self.query.is_empty() {
                actions.push(DialogAction::ReplaceNextInFocused {
                    find: self.query.clone(), replace: self.replace.clone(), regex: self.regex,
                });
            }
            if ui.button("Replace all (focused buffer)").clicked() && !self.query.is_empty() {
                actions.push(DialogAction::ReplaceAllInFocused {
                    find:    self.query.clone(),
                    replace: self.replace.clone(),
                    regex:   self.regex,
                });
            }
            ui.label(
                RichText::new(format!("{} matches", self.results.len()))
                    .size(11.0).color(ui.style().visuals.weak_text_color()),
            );
        });
        ui.separator();

        if self.selected >= self.results.len() { self.selected = 0; }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(280.0)
            .show(ui, |ui| {
                for (i, hit) in self.results.iter().enumerate() {
                    let is_focused = {
                        editor.view_tree.focused_window()
                            .and_then(|wid| editor.view_tree.buffer(wid)) == Some(hit.buf_id)
                    };
                    let label = format!("{}:{}  {}", hit.buf_name, hit.line, hit.text);
                    let color = if i == self.selected {
                        Color32::WHITE
                    } else if is_focused {
                        Color32::LIGHT_GRAY
                    } else {
                        Color32::GRAY
                    };
                    if ui.selectable_label(i == self.selected, RichText::new(label).color(color)).clicked() {
                        self.selected = i;
                        actions.push(DialogAction::FocusBufferAndGoto {
                            buf_id: hit.buf_id,
                            line:   hit.line,
                        });
                    }
                }
            });

        // Enter visits the selected hit and pre-selects the next one, so
        // repeated Enters walk the matches; Shift-Enter walks backwards.
        let enter = query_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let shift_enter = query_resp.lost_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.shift);
        if (enter || shift_enter) && !self.results.is_empty() {
            if shift_enter {
                self.selected = (self.selected + self.results.len() - 1) % self.results.len();
            }
            let hit = &self.results[self.selected];
            actions.push(DialogAction::FocusBufferAndGoto {
                buf_id: hit.buf_id,
                line:   hit.line,
            });
            if enter {
                self.selected = (self.selected + 1) % self.results.len();
            }
            query_resp.request_focus();
        }
        false
    }
}
