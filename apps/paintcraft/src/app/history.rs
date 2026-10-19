//! The history window never navigates by an unguarded positional index.
use super::*;

impl App {
    pub(super) fn studio_history(&mut self, ctx: &egui::Context) {
        if !self.studio.history || self.document_count() == 0 {
            return;
        }
        let view = self.editor.history_view();
        let rows: Vec<_> = view
            .entries
            .iter()
            .map(|e| paintcraft_studio::HistoryRow {
                label: e.label.text(),
                current: e.current,
                redo: e.redo,
            })
            .collect();
        let enabled = !self.editor.gesture_in_progress() && !self.editor.is_editing_text();
        if let Some(index) = paintcraft_studio::history_window(
            ctx,
            &mut self.studio.history,
            &rows,
            view.trimmed_total,
            enabled,
            self.studio.canvas_top,
        ) {
            let entry = &view.entries[index];
            self.request(
                ctx,
                code::HISTORY,
                Action::NavigateHistory {
                    owner: view.ticket.owner,
                    state_id: view.ticket.base.state_id,
                    epoch: view.ticket.base.publication_epoch,
                    history_id: entry.history_id,
                },
            );
        }
    }
}
