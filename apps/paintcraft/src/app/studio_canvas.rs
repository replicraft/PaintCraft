//! Studio owns canvas placement UI; Classic startup preferences stay independent.
use super::*;
use paintcraft_studio::{CanvasResponse, CanvasSize};
use paintcraft_ui::t;
impl App {
    pub(super) fn studio_canvas_settings_dialog(
        &mut self,
        ctx: &egui::Context,
        tab: Uuid,
        mut revision: Option<(Uuid, u64)>,
        mut view: CanvasSize,
    ) {
        if tab != self.active_tab_id()
            || revision.is_some_and(|source| source != self.editor.render_key())
            || (revision.is_some() && self.editor.has_pending_edits())
        {
            self.confirm = None;
            self.editor
                .set_status("Canvas size was not applied: the source document changed.");
            return;
        }
        if revision.is_none() {
            if self.editor.has_pending_edits() {
                let mut cancel = false;
                let modal =
                    egui::Modal::new(egui::Id::unique("studio-canvas-wait")).show(ctx, |ui| {
                        ui.heading(t!(ui.ctx(), "Canvas size"));
                        ui.label(t!(ui.ctx(), "Finishing earlier edits…"));
                        cancel = ui.button(t!(ui.ctx(), "Cancel")).clicked();
                    });
                if ctx.current_pass_index() == 0 {
                    self.confirm = if cancel || modal.should_close() {
                        None
                    } else {
                        Some(Confirm::Canvas(canvas_settings::CanvasDraft::Studio {
                            tab,
                            revision,
                            view,
                        }))
                    };
                }
                return;
            }
            view.size = self.editor.document().canvas;
            revision = Some(self.editor.render_key());
        }
        let response = paintcraft_studio::canvas_size_dialog(ctx, &mut view, settings::SIZE_MAX);
        if ctx.current_pass_index() != 0 {
            return;
        }
        match response {
            CanvasResponse::Cancel => self.confirm = None,
            CanvasResponse::Apply => {
                self.confirm = None;
                if view.size != self.editor.document().canvas {
                    self.request(
                        ctx,
                        code::CANVAS_SIZE,
                        Action::ResizeCanvasAnchored {
                            width: view.size.0,
                            height: view.size.1,
                            anchor: view.anchor,
                        },
                    );
                }
            }
            CanvasResponse::Pending => {
                self.confirm = Some(Confirm::Canvas(canvas_settings::CanvasDraft::Studio {
                    tab,
                    revision,
                    view,
                }))
            }
        }
    }
}

#[cfg(test)]
#[path = "studio_canvas/tests.rs"]
mod tests;
