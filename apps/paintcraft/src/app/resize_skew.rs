//! Staged resize/skew presentation; transform publication remains unavailable.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    tab: Uuid,
    revision: (Uuid, u64),
    view: paintcraft_classic::resize_skew::Draft,
}

impl App {
    pub(super) fn begin_resize_skew(&mut self) {
        self.confirm = Some(Confirm::ResizeSkew(Draft {
            tab: self.active_tab_id(),
            revision: self.editor.render_key(),
            view: paintcraft_classic::resize_skew::Draft::new(self.editor.document().canvas),
        }));
    }

    pub(super) fn resize_skew_dialog(&mut self, ctx: &egui::Context, mut draft: Draft) {
        if draft.tab != self.active_tab_id() || draft.revision != self.editor.render_key() {
            self.confirm = None;
            self.editor
                .set_status("Resize and skew preview closed because the document changed.");
            return;
        }
        let close = paintcraft_classic::resize_skew::dialog_for_skin(
            ctx,
            self.variant(),
            &mut draft.view,
            settings::SIZE_MAX,
        );
        if ctx.current_pass_index() == 0 {
            self.confirm = (!close).then_some(Confirm::ResizeSkew(draft));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stale_resize_preview_closes_without_publishing_changes() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.begin_resize_skew();
        let Some(Confirm::ResizeSkew(draft)) = app.confirm.clone() else {
            panic!("resize preview");
        };
        app.editor.request(8571, Action::AddLayer);
        app.editor.run_pending();
        let snapshot = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        app.resize_skew_dialog(&ctx, draft);
        assert!(app.confirm.is_none());
        assert!(app.editor.document().same_semantics(&snapshot));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert!(app.editor.status().contains("document changed"));
    }
}
