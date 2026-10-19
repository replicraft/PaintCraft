//! Desktop window integration.

use super::*;

impl App {
    /// Lay out one pass of the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let first_pass = ctx.current_pass_index() == 0;
        if first_pass {
            self.poll_dialog(&ctx);
            if !self.dialogs.as_ref().is_some_and(NativeDialogs::pending) {
                self.handle_global_input(ui);
            }
        }
        if self.dialogs.as_ref().is_some_and(NativeDialogs::pending) {
            ctx.request_repaint_after(Duration::from_millis(50));
            ui.disable();
        }
        self.apply_style(&ctx);
        self.sync_texture(&ctx);
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let status = format!(
                    "{}{}",
                    self.editor.status(),
                    if self.editor.is_dirty() {
                        "  •  unsaved changes"
                    } else {
                        ""
                    }
                );
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
                    for zoom in [8u32, 4, 2, 1] {
                        ui.selectable_value(&mut self.zoom, zoom, format!("{zoom}×"));
                    }
                    let (width, height) = self.editor.document().canvas;
                    ui.label(format!("{width} × {height} px"));
                });
            });
        });
        egui::Panel::right("layers")
            .default_size(220.0)
            .show(ui, |ui| {
                self.layer_panel(ui);
            });
        let accept_input = first_pass && !self.dialogs.as_ref().is_some_and(NativeDialogs::pending);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(220, 225, 232))
                    .inner_margin(20),
            )
            .show(ui, |ui| self.canvas(ui, accept_input));
        self.picker_dialog(ui);
        self.recovery_dialog(ui);
        self.confirm_dialog(ui);
        if first_pass && ctx.input(|i| i.viewport().close_requested()) {
            if let Some(saving) = &mut self.saving {
                saving.cancel_restore();
            }
            if self.saving.as_ref().is_some_and(|s| s.pending()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_after_saves = true;
                self.editor
                    .set_status("Waiting for requested saves before closing…");
            } else if !self.editor.is_dirty() && self.editor.checkpoint_info().is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_after_requested_saves(&ctx);
            } else if (self.editor.is_dirty()
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
        if self.recovery_needs_init && !self.close_after_saves && !self.allow_close {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        let now = ctx.input(|i| i.time);
        if now - self.last_autosave >= self.autosave_every.as_secs_f64()
            && !self.allow_close
            && !self.close_after_saves
        {
            self.last_autosave = now;
            if let Some(saving) = &mut self.saving {
                let result = if !self.editor.is_dirty() && self.editor.checkpoint_info().is_some() {
                    saving.clear_recovery(&mut self.editor).map(|()| false)
                } else {
                    saving.checkpoint(&self.editor)
                };
                if let Err(error) = result {
                    self.editor
                        .set_status(format!("Autosave not queued: {error}"));
                }
            } else if self.editor.autosave() {
                self.protect_recovery_source();
                self.pump_recovery_handoff();
            }
        }
        if self.editor.is_dirty() {
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
