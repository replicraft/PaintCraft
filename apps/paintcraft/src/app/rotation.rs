//! Captured scalar staging; Apply queues one ordinary atomic rotation command.
use super::*;
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    tab: Uuid,
    revision: (Uuid, u64),
    owner: Uuid,
    target: Option<Uuid>,
    selection: Selection,
    experience: Experience,
    variant: paintcraft_classic::ClassicVariant,
    view: paintcraft_ui::rotation::Draft,
}
impl App {
    pub(super) fn begin_rotate_custom(&mut self) {
        if !self.action_context().can_rotate_custom {
            return;
        }
        if self.ime_composing || self.editor.has_image_paste() {
            self.editor
                .set_status("Finish pending edits before rotating.");
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
                .set_status("Finish pending edits before rotating.");
            return;
        }
        let selection = self.editor.selection();
        let area =
            match paintcraft_tools::commands::rotate_source(self.editor.document(), &selection) {
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
        match paintcraft_ui::rotation::Draft::new(area) {
            Ok(view) => {
                self.confirm = Some(Confirm::RotateCustom(Draft {
                    tab: self.active_tab_id(),
                    revision: self.editor.render_key(),
                    owner: self.editor.session_owner_id(),
                    target: self.editor.target(),
                    selection,
                    experience: self.experience(),
                    variant: self.variant(),
                    view,
                }))
            }
            Err(error) => self.editor.set_status(error.to_string()),
        }
    }
    fn rotation_current(&self, draft: &Draft) -> bool {
        draft.tab == self.active_tab_id()
            && draft.revision == self.editor.render_key()
            && draft.owner == self.editor.session_owner_id()
            && draft.target == self.editor.target()
            && draft.selection == self.editor.selection()
            && draft.experience == self.experience()
            && draft.variant == self.variant()
            && !self.ime_composing
            && !self.editor.has_image_paste()
            && !self.editor.has_floating_selection()
            && !self.editor.has_pending_edits()
            && self.editor.text_save_ready()
    }
    pub(super) fn rotate_custom_dialog(&mut self, ctx: &egui::Context, mut draft: Draft) {
        if !self.rotation_current(&draft) {
            self.confirm = None;
            self.editor
                .set_status("Rotation preview closed because the source or selection changed.");
            return;
        }
        let response = paintcraft_ui::rotation::dialog(ctx, &mut draft.view);
        if ctx.current_pass_index() == 0 {
            match response {
                paintcraft_ui::rotation::Response::Pending => {
                    self.confirm = Some(Confirm::RotateCustom(draft))
                }
                paintcraft_ui::rotation::Response::Cancel => self.confirm = None,
                paintcraft_ui::rotation::Response::Apply(coefficients) => {
                    self.confirm = None;
                    if !self.rotation_current(&draft) {
                        return;
                    }
                    let clear = if self.experience() == Experience::Studio {
                        Rgba::TRANSPARENT
                    } else {
                        self.editor.canvas_color()
                    };
                    self.request(
                        ctx,
                        paintcraft_ui::actions::Action::RotateCustom.activation_code(),
                        Action::RotateImage(paintcraft_tools::RotateEdit {
                            owner: draft.owner,
                            state_id: draft.revision.0,
                            publication_epoch: draft.revision.1,
                            target: draft.target,
                            selection: draft.selection,
                            coefficients,
                            clear,
                        }),
                    );
                }
            }
        }
    }
}
#[cfg(test)]
#[path = "rotation/tests.rs"]
mod tests;
