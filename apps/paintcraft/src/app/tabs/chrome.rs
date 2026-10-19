//! Workspace tab presentation uses stable identities from the document registry.
use super::*;

impl App {
    pub(crate) fn document_tab_bar(&mut self, ui: &mut egui::Ui, first_pass: bool) {
        let info = self
            .tabs
            .iter()
            .map(|t| {
                let (bound, editor) = t
                    .state
                    .as_ref()
                    .map_or((&self.bound, &self.editor), |s| (&s.bound, &s.editor));
                let label = bound
                    .as_deref()
                    .and_then(Path::file_name)
                    .map(|s| s.to_string_lossy().into_owned())
                    .or_else(|| editor.document().title.clone())
                    .unwrap_or_else(|| format!("Untitled {}", t.number));
                (t.id, t.number, label, editor.is_dirty())
            })
            .collect::<Vec<_>>();
        let rows = info
            .iter()
            .map(
                |(id, number, label, dirty)| paintcraft_studio::DocumentTab {
                    label,
                    number: *number,
                    dirty: *dirty,
                    active: *id == self.active_tab_id(),
                },
            )
            .collect::<Vec<_>>();
        let blocked = self.confirm.is_some()
            || self.tab_closing.is_some()
            || self.close_after_saves
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending);
        let action = ui
            .add_enabled_ui(!blocked, |ui| paintcraft_studio::document_tabs(ui, &rows))
            .inner;
        if !first_pass {
            return;
        }
        let ctx = ui.ctx().clone();
        match action {
            Some(paintcraft_studio::TabAction::Select(i)) => self.activate_tab(info[i].0, &ctx),
            Some(paintcraft_studio::TabAction::Close(i)) => self.request_close_tab(info[i].0, &ctx),
            Some(paintcraft_studio::TabAction::New) => self.new_document(),
            None => {}
        }
    }
}
