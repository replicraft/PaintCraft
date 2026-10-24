//! Staged Classic dialogs. Tool preference changes never publish document history.
use super::*;
use paintcraft_classic::{ColorEditor, ColorResponse};
use paintcraft_core::types::Rgba;
use uuid::Uuid;

#[derive(Debug)]
pub(super) struct ColorDraft {
    document: Uuid,
    source: Rgba,
    secondary: bool,
    editor: ColorEditor,
}

#[derive(Debug)]
pub(super) enum Form {
    Colors(ColorDraft),
}

impl WebApp {
    pub(super) fn begin_colors(&mut self, secondary: bool, slot: Option<u8>) {
        self.editor.cancel();
        let source = if secondary {
            self.controls.secondary
        } else {
            self.controls.primary
        };
        let mut editor = ColorEditor::new(source, self.workspace.custom_colors);
        if let Some(slot) = slot {
            editor = editor.with_selected_custom_slot(usize::from(slot));
        }
        self.form = Some(Form::Colors(ColorDraft {
            document: self.editor.session.document().document_id,
            source,
            secondary,
            editor,
        }));
    }

    pub(super) fn show_form(&mut self, ctx: &egui::Context) {
        let Some(Form::Colors(mut draft)) = self.form.take() else {
            return;
        };
        let current = if draft.secondary {
            self.controls.secondary
        } else {
            self.controls.primary
        };
        if current != draft.source || self.editor.session.document().document_id != draft.document {
            self.message = "Color was not applied: the source document or color changed".into();
            return;
        }
        let response = paintcraft_classic::edit_colors_dialog(ctx, &mut draft.editor);
        if ctx.current_pass_index() != 0 || response == ColorResponse::Pending {
            self.form = Some(Form::Colors(draft));
        } else if response == ColorResponse::Accept {
            if draft.secondary {
                self.controls.secondary = draft.editor.color;
            } else {
                self.controls.primary = draft.editor.color;
            }
            self.workspace.custom_colors = draft.editor.custom;
            self.message = "Color updated".into();
        }
    }
}
