//! Captured resize/skew dialog publishes one ordinary atomic transform command.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    tab: Uuid,
    revision: (Uuid, u64),
    owner: Uuid,
    target: Option<Uuid>,
    view: paintcraft_classic::resize_skew::Draft,
}

impl App {
    pub(super) fn begin_resize_skew(&mut self) {
        if self.ime_composing || self.editor.has_image_paste() {
            self.editor
                .set_status("Finish pending edits before resizing or skewing.");
            return;
        }
        let ctx = self.context.clone();
        self.resolve_tab_input(&ctx);
        self.editor.finish_selection_move();
        self.editor.run_pending();
        if self.ime_composing
            || self.editor.has_image_paste()
            || self.editor.has_floating_selection()
            || self.editor.has_pending_edits()
            || !self.editor.text_save_ready()
        {
            self.editor
                .set_status("Finish pending edits before resizing or skewing.");
            return;
        }
        let area = match paintcraft_tools::commands::stretch_skew_source(
            self.editor.document(),
            &self.editor.selection(),
        ) {
            Ok(Some(area)) => area,
            Ok(None) => {
                self.editor.set_status("The selection is empty.");
                return;
            }
            Err(error) => {
                self.editor.set_status(error.to_string());
                return;
            }
        };

        self.confirm = Some(Confirm::ResizeSkew(Draft {
            tab: self.active_tab_id(),
            revision: self.editor.render_key(),
            owner: self.editor.session_owner_id(),
            target: self.editor.target(),
            view: paintcraft_classic::resize_skew::Draft::new((
                area.width() as u32,
                area.height() as u32,
            )),
        }));
    }

    pub(super) fn resize_skew_dialog(&mut self, ctx: &egui::Context, mut draft: Draft) {
        if draft.tab != self.active_tab_id()
            || draft.revision != self.editor.render_key()
            || draft.owner != self.editor.session_owner_id()
            || draft.target != self.editor.target()
        {
            self.confirm = None;
            self.editor
                .set_status("Resize and skew preview closed because the document changed.");
            return;
        }
        let response = if self.experience() == Experience::Retro {
            let mut response = paintcraft_classic::resize_skew::Response::Pending;
            let modal = egui::Modal::new(egui::Id::unique("native-retro-resize-skew"))
                .frame(paintcraft_retro::frame())
                .show(ctx, |ui| {
                    response = paintcraft_retro::with_style(ui, |ui| {
                        paintcraft_classic::resize_skew::contents(
                            ui,
                            &mut draft.view,
                            settings::SIZE_MAX,
                        )
                    });
                });
            if modal.should_close()
                && response == paintcraft_classic::resize_skew::Response::Pending
            {
                response = paintcraft_classic::resize_skew::Response::Cancel;
            }
            response
        } else {
            paintcraft_classic::resize_skew::dialog_for_skin(
                ctx,
                self.variant(),
                &mut draft.view,
                settings::SIZE_MAX,
            )
        };
        if ctx.current_pass_index() == 0 {
            use paintcraft_classic::resize_skew::Response;
            match response {
                Response::Pending => self.confirm = Some(Confirm::ResizeSkew(draft)),
                Response::Cancel => self.confirm = None,
                Response::Apply(p) => {
                    self.confirm = None;
                    if self.editor.has_pending_edits()
                        || !self.editor.text_save_ready()
                        || self.editor.has_image_paste()
                    {
                        self.editor
                            .set_status("Finish pending edits before resizing or skewing.");
                        return;
                    }
                    let clear = if self.experience() == Experience::Studio {
                        Rgba::TRANSPARENT
                    } else {
                        self.editor.canvas_color()
                    };
                    self.request(
                        ctx,
                        paintcraft_ui::actions::Action::ResizeSkew.activation_code(),
                        Action::StretchSkewImage(paintcraft_tools::StretchSkewEdit {
                            owner: draft.owner,
                            state_id: draft.revision.0,
                            publication_epoch: draft.revision.1,
                            target: draft.target,
                            width: p.width,
                            height: p.height,
                            shear: p.shear,
                            clear,
                        }),
                    );
                }
            }
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
    #[test]
    fn ime_composition_refuses_before_resolving_or_publishing_input() {
        let mut app = App::new(None);
        app.ime_composing = true;
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        app.begin_resize_skew();
        assert!(app.confirm.is_none());
        assert!(app.editor.document().same_semantics(&before));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert!(app.ime_composing);
    }
}
