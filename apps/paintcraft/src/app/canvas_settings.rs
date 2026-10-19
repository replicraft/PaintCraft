//! Classic canvas dimensions and New-canvas policy use separate staged dialogs.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CanvasDraft {
    Defaults(settings::CanvasDefaults),
    Resize {
        tab: Uuid,
        revision: (Uuid, u64),
        size: (u32, u32),
    },
}

impl App {
    pub(super) fn remember_canvas_size(&mut self, size: (u32, u32)) {
        if self.settings.canvas.last != Some(size) {
            self.settings.canvas.last = Some(size);
            self.persist_settings();
        }
    }

    pub(super) fn begin_canvas_dialog(&mut self, ctx: &egui::Context, defaults: bool) {
        self.resolve_tab_input(ctx);
        let draft = if defaults {
            CanvasDraft::Defaults(self.settings.canvas.clone())
        } else {
            CanvasDraft::Resize {
                tab: self.active_tab_id(),
                revision: self.editor.render_key(),
                size: self.editor.document().canvas,
            }
        };
        self.confirm = Some(Confirm::Canvas(draft));
    }

    pub(super) fn canvas_settings_dialog(&mut self, ctx: &egui::Context, mut draft: CanvasDraft) {
        if let CanvasDraft::Resize { tab, revision, .. } = &draft
            && (*tab != self.active_tab_id() || *revision != self.editor.render_key())
        {
            self.confirm = None;
            self.editor
                .set_status("Canvas size was not applied: the source document changed.");
            return;
        }
        let defaults = matches!(draft, CanvasDraft::Defaults(_));
        let title = if defaults {
            "Default canvas size"
        } else {
            "Canvas size"
        };
        let mut apply = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique(title)).show(ctx, |ui| {
            ui.heading(title);
            let size = match &mut draft {
                CanvasDraft::Defaults(prefs) => &mut prefs.configured,
                CanvasDraft::Resize { size, .. } => size,
            };
            for (label, value) in [("Width", &mut size.0), ("Height", &mut size.1)] {
                ui.horizontal(|ui| {
                    let caption = ui.label(label);
                    ui.add(
                        egui::DragValue::new(value)
                            .range(1..=settings::SIZE_MAX)
                            .suffix(" px"),
                    )
                    .labelled_by(caption.id);
                });
            }
            if let CanvasDraft::Defaults(prefs) = &mut draft {
                ui.radio_value(&mut prefs.remember_last, true, "Remember last canvas size");
                ui.radio_value(
                    &mut prefs.remember_last,
                    false,
                    "Use configured default size",
                );
                if let Some((w, h)) = prefs.last {
                    ui.weak(format!("Last canvas: {w} × {h} pixels"));
                }
                ui.weak("Applies to new Classic documents. The current image stays as it is.");
            } else {
                ui.weak("Keep the top-left corner. Content outside the canvas is preserved.");
            }
            ui.horizontal(|ui| {
                let size = match &draft {
                    CanvasDraft::Defaults(prefs) => prefs.configured,
                    CanvasDraft::Resize { size, .. } => *size,
                };
                let valid = (1..=settings::SIZE_MAX).contains(&size.0)
                    && (1..=settings::SIZE_MAX).contains(&size.1);
                apply = ui.add_enabled(valid, egui::Button::new("Apply")).clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if ctx.current_pass_index() != 0 {
            return;
        }
        if cancel || response.should_close() {
            self.confirm = None;
        } else if apply {
            self.confirm = None;
            match draft {
                CanvasDraft::Defaults(prefs) => {
                    self.settings.canvas = prefs;
                    self.persist_settings();
                }
                CanvasDraft::Resize { size, .. } => {
                    let before = self.editor.document().canvas;
                    self.request(
                        ctx,
                        code::CANVAS_SIZE,
                        Action::ResizeCanvas {
                            width: size.0,
                            height: size.1,
                        },
                    );
                    self.editor.run_pending();
                    if self.editor.document().canvas == size && size != before {
                        self.remember_canvas_size(size);
                    }
                }
            }
        } else {
            self.confirm = Some(Confirm::Canvas(draft));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resize_dialog_rejects_a_stale_document_revision() {
        let mut app = App::new(None);
        let ctx = egui::Context::default();
        app.begin_canvas_dialog(&ctx, false);
        let Some(Confirm::Canvas(draft)) = app.confirm.clone() else {
            panic!("canvas draft");
        };
        app.editor.request(12345, Action::AddLayer);
        app.editor.run_pending();
        let before = app.editor.document_snapshot();
        app.canvas_settings_dialog(&ctx, draft);
        assert!(app.confirm.is_none());
        assert!(app.editor.document().same_semantics(&before));
        assert!(app.editor.status().contains("source document changed"));
    }
}
