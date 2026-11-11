//! Angle and bounds are staged; only Apply submits a captured atomic edit.
use super::{Form, WebApp};
use eframe::egui;
use paintcraft_classic::ClassicVariant;
use paintcraft_core::types::Rgba;
use paintcraft_editor::session::HistoryTicket;
use paintcraft_raster::Selection;
use paintcraft_tools::{Action as Edit, RotateEdit, commands};
use paintcraft_ui::{
    actions::Frontend,
    rotation::{Draft, Response},
};
use uuid::Uuid;

#[derive(Debug)]
pub(in crate::app) struct Captured {
    ticket: HistoryTicket,
    frontend: Frontend,
    variant: ClassicVariant,
    target: Option<Uuid>,
    selection: Selection,
    clear: Rgba,
    view: Draft,
}

impl WebApp {
    pub(in crate::app) fn can_rotate_custom(&self) -> bool {
        (self.frontend == Frontend::Studio
            || (self.frontend == Frontend::Classic && self.variant == ClassicVariant::V11))
            && !self.blocked()
            && !self.paste.is_placing()
            && self.editor.tools.floating_selection().is_none()
            && !self.editor.tools.is_editing_text()
    }

    pub(in crate::app) fn begin_rotate(&mut self) {
        if !self.can_rotate_custom() {
            self.message = "Finish pending edits before rotating.".into();
            return;
        }
        let selection = self.editor.session.selection();
        let result = commands::rotate_source(self.editor.session.document(), &selection)
            .and_then(|source| source.map(Draft::new).transpose());
        match result {
            Ok(Some(view)) => {
                self.editor.tools.cancel_gesture();
                self.form = Some(Form::Rotate(Captured {
                    ticket: self.editor.session.history_ticket(),
                    frontend: self.frontend,
                    variant: self.variant,
                    target: self.editor.target,
                    selection,
                    clear: self.clear_color(),
                    view,
                }));
            }
            Ok(None) => self.message = "The selection is empty.".into(),
            Err(error) => self.message = error.to_string(),
        }
    }

    pub(super) fn rotation_form(&mut self, ctx: &egui::Context, mut draft: Captured) {
        if draft.ticket != self.editor.session.history_ticket()
            || draft.target != self.editor.target
            || draft.selection != self.editor.session.selection()
            || draft.frontend != self.frontend
            || draft.variant != self.variant
            || !self.can_rotate_custom()
        {
            self.message =
                "Rotation preview closed because the source or selection changed.".into();
            return;
        }
        let response = paintcraft_ui::rotation::dialog(ctx, &mut draft.view);
        if ctx.current_pass_index() != 0 || response == Response::Pending {
            self.form = Some(Form::Rotate(draft));
        } else if let Response::Apply(coefficients) = response {
            self.edit(Edit::RotateImage(RotateEdit {
                owner: draft.ticket.owner,
                state_id: draft.ticket.base.state_id,
                publication_epoch: draft.ticket.base.publication_epoch,
                target: draft.target,
                selection: draft.selection,
                coefficients,
                clear: draft.clear,
            }));
        }
    }
}

#[cfg(test)]
mod tests;
