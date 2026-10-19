//! Desktop window integration.

use super::*;

impl App {
    /// Lay out one pass of the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.context = ctx.clone();
        let first_pass = ctx.current_pass_index() == 0;
        if first_pass {
            self.poll_dialog(&ctx);
            if !self.closing_workspace
                && self.confirm.is_none()
                && self.tab_closing.is_none()
                && !self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            {
                self.handle_global_input(ui);
            }
        }
        if self.closing_workspace
            || self.confirm.is_some()
            || self.tab_closing.is_some()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            ctx.request_repaint_after(Duration::from_millis(50));
            ui.disable();
        }
        self.apply_style(&ctx);
        if self.document_count() > 0 {
            self.sync_texture(&ctx);
        }
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        if self.experience() == Experience::Studio || self.document_count() > 1 {
            egui::Panel::top("documents").show(ui, |ui| {
                if self.experience() == Experience::Classic {
                    ui.weak("PaintCraft workspace documents");
                }
                self.document_tab_bar(ui, first_pass);
            });
        }
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
                    for zoom in [8u32, 4, 2, 1] {
                        ui.selectable_value(&mut self.zoom, zoom, format!("{zoom}×"));
                    }
                    let (width, height) = self.editor.document().canvas;
                    ui.label(format!("{width} × {height} px"));
                });
            });
        });
        if self.experience() == Experience::Classic {
            egui::Panel::right("layers")
                .default_size(220.0)
                .show(ui, |ui| {
                    self.layer_panel(ui);
                    self.incomplete_save_panel(ui);
                });
        } else if self
            .saving
            .as_ref()
            .is_some_and(saving::NativeSaving::failed)
        {
            // Save diagnostics remain reachable even when the Layers window is hidden.
            egui::Panel::bottom("studio-save-status").show(ui, |ui| self.incomplete_save_panel(ui));
        }
        let accept_input = first_pass && !self.dialogs.as_ref().is_some_and(NativeDialogs::pending);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(if self.experience() == Experience::Studio {
                        Color32::from_rgb(191, 195, 200)
                    } else {
                        Color32::from_rgb(220, 225, 232)
                    })
                    .inner_margin(20),
            )
            .show(ui, |ui| {
                if self.document_count() > 0 {
                    self.canvas(ui, accept_input && self.tab_closing.is_none());
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("Open an image or create a new document.")
                    });
                }
            });
        if self.experience() == Experience::Studio {
            self.studio_windows(&ctx);
        }
        self.picker_dialog(ui);
        self.recovery_dialog(ui);
        self.confirm_dialog(ui);
        self.tab_close_dialog(&ctx);
        if first_pass
            && (self.tab_closing.is_some() || self.confirm.is_some())
            && ctx.input(|i| i.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if first_pass
            && self.tab_closing.is_none()
            && self.confirm.is_none()
            && ctx.input(|i| i.viewport().close_requested())
        {
            self.resolve_tab_input(&ctx);
            if let Some(saving) = &mut self.saving {
                saving.cancel_restore();
            }
            if self.saving.as_ref().is_some_and(|s| s.pending()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_after_saves = true;
                self.editor
                    .set_status("Waiting for requested saves before closing…");
            } else if !self.any_dirty_documents() && self.editor.checkpoint_info().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_after_requested_saves(&ctx);
            } else if (self.any_dirty_documents()
                || self
                    .saving
                    .as_ref()
                    .is_some_and(saving::NativeSaving::failed))
                && !self.allow_close
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.confirm = Some(Confirm::QuitUnsaved);
            }
        }
        if self.editor.run_pending() > 0 {
            ctx.request_repaint();
            self.sync_texture(&ctx);
        }
        self.finish_save();
        self.poll_native_saves(&ctx);
        self.pump_tab_close(&ctx);
        if self.recovery_needs_init && !self.close_after_saves && !self.allow_close {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        self.tick_documents(&ctx);
        if self.any_dirty_documents() {
            ctx.request_repaint_after(self.autosave_every.max(Duration::from_millis(250)));
        }
        if let Some(n) = self.exit_after_frames {
            // egui repaints only on events; a smoke run must keep frames coming.
            ctx.request_repaint();
            if ctx.cumulative_frame_nr() >= n {
                self.close_after_requested_saves(&ctx);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        App::ui(self, ui);
    }
}
