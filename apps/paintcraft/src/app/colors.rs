//! Color dialogs edit tool preferences, with ownership retained across passes.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ColorDraft {
    tab: Uuid,
    secondary: bool,
    source: Rgba,
    editor: paintcraft_classic::ColorEditor,
}

impl App {
    pub(super) fn begin_custom_color_dialog(
        &mut self,
        ctx: &egui::Context,
        slot: u8,
        secondary: bool,
    ) {
        self.begin_color_dialog(ctx, secondary);
        if let Some(Confirm::Colors(draft)) = &mut self.confirm {
            draft.editor = draft
                .editor
                .clone()
                .with_selected_custom_slot(usize::from(slot));
        }
    }

    pub(super) fn begin_color_dialog(&mut self, ctx: &egui::Context, secondary: bool) {
        self.resolve_tab_input(ctx);
        let source = if secondary {
            self.editor.secondary
        } else {
            self.editor.color
        };
        self.confirm = Some(Confirm::Colors(ColorDraft {
            tab: self.active_tab_id(),
            secondary,
            source,
            editor: paintcraft_classic::ColorEditor::new(source, self.settings.custom_colors),
        }));
    }

    pub(super) fn classic_color_dialog(&mut self, ctx: &egui::Context, mut draft: ColorDraft) {
        let current = if draft.secondary {
            self.editor.secondary
        } else {
            self.editor.color
        };
        if draft.tab != self.active_tab_id() || draft.source != current {
            self.confirm = None;
            self.editor
                .set_status("Color was not applied: the source document or color changed.");
            return;
        }
        let result = paintcraft_classic::edit_colors_dialog(ctx, &mut draft.editor);
        if ctx.current_pass_index() != 0 {
            return;
        }
        match result {
            paintcraft_classic::ColorResponse::Pending => {
                self.confirm = Some(Confirm::Colors(draft))
            }
            paintcraft_classic::ColorResponse::Cancel => self.confirm = None,
            paintcraft_classic::ColorResponse::Accept => {
                self.confirm = None;
                if draft.secondary {
                    self.editor.secondary = draft.editor.color;
                } else {
                    self.editor.color = draft.editor.color;
                }
                if self.settings.custom_colors != draft.editor.custom {
                    self.settings.custom_colors = draft.editor.custom;
                    self.persist_settings();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_color_dialog_cannot_edit_another_document_or_overwrite_newer_preferences() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.begin_color_dialog(&ctx, false);
        let Some(Confirm::Colors(draft)) = app.confirm.clone() else {
            panic!("color draft");
        };
        app.editor.color = Rgba([12, 34, 56, 93]);
        app.classic_color_dialog(&ctx, draft);
        assert!(app.confirm.is_none());
        assert_eq!(app.editor.color, Rgba([12, 34, 56, 93]));
        app.begin_color_dialog(&ctx, false);
        let Some(Confirm::Colors(mut draft)) = app.confirm.clone() else {
            panic!("color draft");
        };
        draft.tab = Uuid::nil();
        app.classic_color_dialog(&ctx, draft);
        assert!(app.confirm.is_none());
        assert_eq!(app.editor.color, Rgba([12, 34, 56, 93]));
    }
}
