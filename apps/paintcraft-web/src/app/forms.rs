//! Staged Classic dialogs. Tool preference changes never publish document history.
use super::*;
use paintcraft_classic::{ColorEditor, ColorResponse};
use paintcraft_core::types::Rgba;
use uuid::Uuid;

#[derive(Debug)]
pub(super) struct ColorDraft {
    owner: Uuid,
    source: Rgba,
    secondary: bool,
    editor: ColorEditor,
}

#[derive(Debug)]
pub(super) enum Form {
    Colors(ColorDraft),
    Canvas(CanvasDraft),
}

#[derive(Debug)]
pub(super) struct CanvasDraft {
    ticket: paintcraft_editor::session::HistoryTicket,
    view: paintcraft_classic::canvas_size::CanvasSize,
    properties: Option<String>,
}

impl WebApp {
    pub(super) fn begin_colors(&mut self, secondary: bool, slot: Option<u8>) {
        self.editor.tools.cancel_gesture();
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
            owner: self.editor.session.history_ticket().owner,
            source,
            secondary,
            editor,
        }));
    }

    pub(super) fn begin_canvas(&mut self, action: Action) {
        let defaults = action == Action::CanvasDefaults;
        let properties = action == Action::ImageProperties;
        self.editor.tools.cancel_gesture();
        self.form = Some(Form::Canvas(CanvasDraft {
            ticket: self.editor.session.history_ticket(),
            view: paintcraft_classic::canvas_size::CanvasSize {
                size: if defaults {
                    self.preferences.canvas.configured
                } else {
                    self.editor.session.document().canvas
                },
                startup: defaults.then_some(paintcraft_classic::canvas_size::StartupSize {
                    remember_last: self.preferences.canvas.remember_last,
                    last: self.preferences.canvas.last,
                }),
            },
            properties: properties.then(|| self.name.clone()),
        }));
    }

    fn canvas_form(&mut self, ctx: &egui::Context, mut draft: CanvasDraft) {
        use paintcraft_classic::canvas_size::{CanvasResponse, canvas_size_dialog_for_skin};
        if draft.ticket != self.editor.session.history_ticket() {
            self.message = "Dimensions were not applied: the source document changed".into();
            return;
        }
        let result = canvas_size_dialog_for_skin(
            ctx,
            self.variant,
            &mut draft.view,
            paintcraft_document::CANVAS_SIDE_MAX,
            Some(crate::editor::MAX_PIXELS),
            draft.properties.as_deref(),
        );
        if ctx.current_pass_index() != 0 || result == CanvasResponse::Pending {
            self.form = Some(Form::Canvas(draft));
        } else if result == CanvasResponse::Apply {
            let (width, height) = draft.view.size;
            if let Some(startup) = draft.view.startup {
                self.preferences.canvas.configured = (width, height);
                self.preferences.canvas.remember_last = startup.remember_last;
                self.message = "Default canvas preferences updated".into();
                self.persist_preferences();
            } else {
                let before = self.editor.session.document().canvas;
                self.edit(Edit::ResizeCanvas { width, height });
                if before != (width, height)
                    && self.editor.session.document().canvas == (width, height)
                {
                    self.remember_canvas_size((width, height));
                }
            }
        }
    }

    pub(super) fn show_form(&mut self, ctx: &egui::Context) {
        match self.form.take() {
            Some(Form::Colors(draft)) => self.color_form(ctx, draft),
            Some(Form::Canvas(draft)) => self.canvas_form(ctx, draft),
            None => {}
        }
    }

    fn color_form(&mut self, ctx: &egui::Context, mut draft: ColorDraft) {
        let current = if draft.secondary {
            self.controls.secondary
        } else {
            self.controls.primary
        };
        if current != draft.source || self.editor.session.history_ticket().owner != draft.owner {
            self.message = "Color was not applied: the source document or color changed".into();
            return;
        }
        let response =
            paintcraft_classic::edit_colors_dialog_for_skin(ctx, self.variant, &mut draft.editor);
        if ctx.current_pass_index() != 0 || response == ColorResponse::Pending {
            self.form = Some(Form::Colors(draft));
        } else if response == ColorResponse::Accept {
            if draft.secondary {
                self.controls.secondary = draft.editor.color;
            } else {
                self.controls.primary = draft.editor.color;
            }
            self.workspace.custom_colors = draft.editor.custom;
            self.preferences.custom_colors = draft.editor.custom;
            self.message = "Color updated".into();
            self.persist_preferences();
        }
    }
}
