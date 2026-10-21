//! Desktop window integration.

use super::*;

impl App {
    /// Lay out one pass of the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.context = ctx.clone();
        let first_pass = ctx.current_pass_index() == 0;
        if first_pass {
            self.poll_edits(&ctx);
            if let Some(fullscreen) = ctx.input(|i| i.viewport().fullscreen) {
                self.fullscreen = fullscreen;
            }
            self.editor.set_text_style(self.settings.text.clone());
            self.poll_fonts();
            #[cfg(target_os = "macos")]
            self.poll_native_menu(&ctx);
            self.poll_dialog(&ctx);
            if !self.closing_workspace
                && self.confirm.is_none()
                && self.tab_closing.is_none()
                && !self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            {
                self.handle_global_input(ui);
            }
        }
        if self.switching.is_some()
            || self.closing_workspace
            || self.close_after_edits
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
        if self.experience() == Experience::Studio || self.settings.classic_status_bar {
            self.status_bar(ui);
        }
        if self.experience() == Experience::Classic && self.classic.layers {
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
            egui::Panel::right("workspace-save-status")
                .default_size(320.0)
                .show(ui, |ui| self.incomplete_save_panel(ui));
        }
        if self.action_context().blocked {
            // A modal may consume the eventual release. Retire the gesture now
            // so it cannot resume or publish after the dialog is dismissed.
            self.editor.cancel_gesture();
        }
        let accept_input =
            first_pass && !self.action_context().blocked && !self.editor.has_pending_edits();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(paintcraft_ui::theme::workspace(ctx.theme()))
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
            if self.any_pending_edits() || self.any_deferred_files() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_after_edits = true;
                self.editor
                    .set_status("Waiting for requested edits before closing…");
            } else if self.saving.as_ref().is_some_and(|s| s.pending()) {
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
        self.poll_edits(&ctx);
        self.finish_save();
        self.poll_native_saves(&ctx);
        self.pump_tab_close(&ctx);
        if first_pass {
            self.pump_interface_switch(&ctx);
        }
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
