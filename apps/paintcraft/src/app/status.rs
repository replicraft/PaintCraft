//! Document status and zoom presentation.
use super::*;

impl App {
    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let status = if self.document_count() == 0 {
                    "No document open".to_owned()
                } else {
                    format!(
                        "{}{}",
                        self.editor.status(),
                        if self.editor.is_dirty() {
                            "  •  unsaved changes"
                        } else {
                            ""
                        }
                    )
                };
                let response = ui.add(egui::Label::new(status).truncate());
                if let Some(info) = self.editor.checkpoint_info() {
                    let when = info.completed.elapsed().map_or_else(
                        |_| "at a recorded earlier time".to_owned(),
                        |elapsed| format!("{} seconds ago", elapsed.as_secs()),
                    );
                    response.on_hover_text(format!(
                        "Recovery checkpoint {} confirmed {}. {}",
                        info.sequence,
                        when,
                        if info.state_id == self.editor.document().state_id {
                            "This checkpoint contains the current document."
                        } else {
                            "Newer edits are not in this checkpoint yet."
                        }
                    ));
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
        });
    }
}
