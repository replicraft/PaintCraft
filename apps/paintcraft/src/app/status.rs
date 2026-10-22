//! Document status and zoom presentation.
use super::*;

impl App {
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) -> Option<egui::Rect> {
        let ctx = ui.ctx().clone();
        egui::Panel::bottom("status")
            .show(ui, |ui| {
                if self.experience() == Experience::Classic && self.variant() == ClassicVariant::V11
                {
                    let size = Vec2::new(
                        ui.available_width(),
                        paintcraft_classic::status::height(ui, self.document_count() > 0),
                    );
                    return Some(ui.allocate_space(size).1);
                }
                let status = self.status_message();
                let recovery = self.checkpoint_tooltip();
                ui.horizontal(|ui| {
                    let response = ui.add(egui::Label::new(status).truncate());
                    if let Some(recovery) = recovery {
                        response.on_hover_text(recovery);
                    }
                    if let Some((x, y)) = self.hover_doc {
                        ui.separator();
                        ui.label(format!("{x}, {y}"));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.document_count() == 0 {
                            return;
                        }
                        if self.experience() == Experience::Classic {
                            let context = self.action_context();
                            let actions = paintcraft_classic::zoom_controls(
                                ui,
                                context,
                                egui::Id::unique(self.active_tab_id()),
                            );
                            self.dispatch_chrome(&ctx, actions);
                        } else {
                            for (zoom, action) in [
                                (8.0, paintcraft_ui::actions::ViewAction::Octuple),
                                (4.0, paintcraft_ui::actions::ViewAction::Quadruple),
                                (2.0, paintcraft_ui::actions::ViewAction::Double),
                                (1.0, paintcraft_ui::actions::ViewAction::ActualSize),
                            ] {
                                if ui
                                    .selectable_label(self.zoom == zoom, format!("{zoom}×"))
                                    .clicked()
                                {
                                    self.dispatch_chrome(
                                        &ctx,
                                        vec![paintcraft_ui::actions::Action::View(action)],
                                    );
                                }
                            }
                            ui.label(format!("{:.2}%", self.zoom * 100.0));
                        }
                        let (width, height) = self.editor.document().canvas;
                        ui.label(format!("{width} × {height} px"));
                    });
                });
                None
            })
            .inner
    }

    pub(super) fn classic_status_contents(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let status = self.status_message();
        let recovery = self.checkpoint_tooltip();
        ui.scope_builder(
            egui::UiBuilder::new()
                .scope_id(egui::Id::unique(("classic-status", self.active_tab_id())))
                .max_rect(rect),
            |ui| {
                let selection = match self.editor.selection() {
                    Selection::Rect(rect) => Some((rect.width(), rect.height())),
                    Selection::Empty => Some((0, 0)),
                    Selection::Inactive => None,
                };
                let model = paintcraft_classic::status::Status {
                    message: &status,
                    recovery: recovery.as_deref(),
                    canvas: (self.document_count() > 0).then_some(self.editor.document().canvas),
                    pointer: self.hover_doc,
                    selection,
                };
                let actions = paintcraft_classic::status::contents(
                    ui,
                    model,
                    self.action_context(),
                    egui::Id::unique(self.active_tab_id()),
                );
                self.dispatch_chrome(ui.ctx(), actions);
            },
        );
    }

    fn status_message(&self) -> String {
        if self.document_count() == 0 {
            return "No document open".to_owned();
        }
        format!(
            "{}{}",
            self.editor.status(),
            if self.editor.is_dirty() {
                "  •  unsaved changes"
            } else {
                ""
            }
        )
    }

    fn checkpoint_tooltip(&self) -> Option<String> {
        let info = self.editor.checkpoint_info()?;
        let when = info.completed.elapsed().map_or_else(
            |_| "at a recorded earlier time".to_owned(),
            |elapsed| format!("{} seconds ago", elapsed.as_secs()),
        );
        Some(format!(
            "Recovery checkpoint {} confirmed {}. {}",
            info.sequence,
            when,
            if info.state_id == self.editor.document().state_id {
                "This checkpoint contains the current document."
            } else {
                "Newer edits are not in this checkpoint yet."
            }
        ))
    }
}
