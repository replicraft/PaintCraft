//! Save-confirmed, admission-frozen restart. Installation never uses Quit anyway.
use super::*;
use paintcraft_ui::t;
use std::{sync::Mutex, thread::JoinHandle};

pub(super) enum State {
    Prompt {
        settings: Option<preferences::SettingsDraft>,
    },
    Saving {
        origin: Uuid,
        settings: Option<preferences::SettingsDraft>,
    },
    Draining(Option<retirement::Drain>),
    Applying(ApplyWorker),
    Failed(String),
    Closing,
}
struct ApplyInputs {
    guard: Option<paintcraft_update::InstallLock>,
    verified: Option<paintcraft_update::Verified>,
    result: Option<paintcraft_update::Result<()>>,
}
impl Drop for ApplyInputs {
    fn drop(&mut self) {
        // The original process lease survives abandoned UI/worker ownership.
        // OS process teardown closes this deliberately retained descriptor.
        if let Some(guard) = self.guard.take() {
            std::mem::forget(guard);
        }
    }
}
pub(super) struct ApplyWorker {
    inputs: Arc<Mutex<ApplyInputs>>,
    thread: JoinHandle<()>,
}

impl App {
    fn update_external_busy(&self) -> bool {
        self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            || self.printer.pending()
            || self.clipboard.retirement_busy()
            || self.paste.retirement_busy()
            || self.ordinary_output.retirement_busy()
            || self.fonts.retirement_busy()
            || self.pending_import.is_some()
            || self.jxr_export.is_some()
            || self.dds_export.is_some()
            || self.msp_export.is_some()
    }
    fn update_parked_idle(&self) -> bool {
        self.parked_workspaces
            .values()
            .all(|workspace| workspace.update_restart_idle())
    }
    fn update_text_failed(&self) -> bool {
        self.editor.failed_text_retained()
            || self.tabs.iter().any(|tab| {
                tab.state
                    .as_ref()
                    .is_some_and(|state| state.editor.failed_text_retained())
            })
    }
    pub(super) fn begin_update_restart(&mut self, ctx: &egui::Context) {
        if self.update_restart.is_some()
            || !self.updates.verified_ready()
            || self.installation_lock.is_none()
            || (self.confirm.is_some() && !matches!(self.confirm, Some(Confirm::Settings(_))))
            || self.save_all.is_some()
            || self.switching.is_some()
            || self.tab_closing.is_some()
            || self.tab_save_close.is_some()
            || self.tab_close_wait.is_some()
            || self.closing_workspace
            || self.allow_close
            || self.close_after_saves
            || self.close_after_edits
        {
            return;
        }
        if self.update_external_busy() {
            self.updates.set_restart_error(t!(ctx, "Finish active dialogs, printing, clipboard, paste, export, and font loading before restarting."));
        } else if !self.update_parked_idle() {
            self.updates.set_restart_error(t!(
                ctx,
                "The parked workspace must be clean and idle before restarting."
            ));
        } else {
            let settings = match self.confirm.take() {
                Some(Confirm::Settings(draft)) => Some(draft),
                _ => None,
            };
            self.update_restart = Some(State::Prompt { settings });
        }
        ctx.request_repaint();
    }
    pub(super) fn cancel_update_restart_after_save_failure(&mut self) {
        if matches!(self.update_restart, Some(State::Saving { .. })) {
            self.restore_update_settings();
        }
    }
    fn restore_update_settings(&mut self) {
        let settings = match self.update_restart.take() {
            Some(State::Saving { origin, settings }) => {
                self.save_all = None;
                self.swap_tab_state(origin);
                self.reset_tab_presentation();
                settings
            }
            Some(State::Prompt { settings }) => settings,
            _ => None,
        };
        if let Some(settings) = settings {
            self.confirm = Some(Confirm::Settings(settings));
        }
    }
    fn cancel_update_restart(&mut self, ctx: &egui::Context) {
        self.restore_update_settings();
        self.editor.set_status(t!(
            ctx,
            "Update restart cancelled; your documents remain open."
        ));
    }
    fn begin_update_saves(&mut self, ctx: &egui::Context) {
        if self.update_external_busy() || !self.update_parked_idle() || self.update_text_failed() {
            self.cancel_update_restart(ctx);
            return;
        }
        let settings = match self.update_restart.take() {
            Some(State::Prompt { settings }) => settings,
            _ => None,
        };
        self.update_restart = Some(State::Saving {
            origin: self.active_tab_id(),
            settings,
        });
        if !self.start_save_all_for_update(ctx) {
            self.cancel_update_restart(ctx);
        }
    }
    fn pump_update_saves(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.poll_edits(&ctx);
        if self.editing.is_none() {
            // The synchronous fallback still owns accepted commands and saves.
            // Normal canvas frames are paused, so drain those original queues here.
            let original = self.active_tab_id();
            let ids = self.tabs.iter().map(|tab| tab.id).collect::<Vec<_>>();
            for id in ids {
                if self.swap_tab_state(id) {
                    self.editor.run_pending();
                    self.finish_save();
                    self.remember_published_canvas_size();
                    self.drop_revoked_text_files();
                }
            }
            self.swap_tab_state(original);
        }
        self.finish_save();
        self.poll_native_saves(&ctx);
        self.poll_dialog(&ctx);
        self.confirm_dialog(ui);
        if self.update_text_failed() {
            self.cancel_update_restart(&ctx);
            return;
        }
        self.pump_save_all(&ctx);
        if self.update_restart.is_none() {
            return;
        }
        if self.save_all.is_none() {
            if self.any_dirty_documents()
                || !self.update_parked_idle()
                || self.any_pending_edits()
                || self.any_deferred_files()
                || self
                    .saving
                    .as_ref()
                    .is_some_and(|s| s.failed() || s.pending())
                || self.update_external_busy()
            {
                self.cancel_update_restart(&ctx);
                self.updates.set_restart_error(t!(&ctx, "Saving was cancelled, failed, or newer edits remain unsaved. Update not installed."));
            } else {
                self.update_restart = Some(State::Draining(None));
            }
        }
    }
    fn drain_update_workers(&mut self, ctx: &egui::Context) {
        let mut jobs: Vec<retirement::Shutdown> = Vec::new();
        let mut error = None;
        if let Some(service) = self.editing.take() {
            match service.retire_for_update() {
                Ok(job) => jobs.push(job),
                Err(e) => error = Some(e),
            }
        }
        self.pending_frame = None;
        if let Some(renderer) = self.renderer.take() {
            jobs.push(Box::new(move || renderer.shutdown()));
        }
        jobs.push(self.thumbnail.retire_for_update());
        jobs.push(self.layer_thumbnails.retire_for_update());
        if let Some(driver) = self.gpu_completion.take() {
            jobs.push(Box::new(move || driver.shutdown()));
        }
        if let Some(dialogs) = self.dialogs.take() {
            jobs.push(Box::new(move || dialogs.shutdown()));
        }
        let printer = std::mem::take(&mut self.printer);
        jobs.push(Box::new(move || {
            printer.shutdown().map_err(|e| {
                paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::StorageFailed,
                    e,
                )
            })
        }));
        let font_threads = self.fonts.retire_for_update();
        jobs.push(Box::new(move || font_threads.shutdown()));
        match self.ordinary_output.retire_for_update() {
            Ok(threads) => jobs.push(Box::new(move || threads.shutdown())),
            Err(e) => {
                error.get_or_insert(e);
            }
        }
        let (threads, writer) = self.clipboard.retire_for_update();
        jobs.push(Box::new(move || {
            let captured = threads.shutdown();
            let written = writer.map_or(Ok(()), |writer| {
                writer.shutdown().map_err(|e| {
                    paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::StorageFailed,
                        e,
                    )
                })
            });
            captured.and(written)
        }));
        let (threads, reader) = self.paste.retire_for_update();
        jobs.push(Box::new(move || {
            let decoded = threads.shutdown();
            let read = reader.map_or(Ok(()), |reader| {
                reader.shutdown().map_err(|e| {
                    paintcraft_core::types::Error::new(
                        paintcraft_core::types::ErrorCode::StorageFailed,
                        e,
                    )
                })
            });
            decoded.and(read)
        }));
        if let Some(saving) = self.saving.take() {
            jobs.push(Box::new(move || {
                drop(saving);
                Ok(())
            }));
        }
        if let Some(error) = error {
            jobs.push(Box::new(move || Err(error)));
        }
        self.update_restart = match retirement::Drain::start(jobs, ctx) {
            Ok(drain) => Some(State::Draining(Some(drain))),
            Err(error) => Some(State::Failed(format!(
                "{}\n{error}",
                t!(
                    ctx,
                    "Background shutdown failed. The update was not installed."
                )
            ))),
        };
    }
    fn start_update_apply(&mut self, ctx: &egui::Context) {
        if self.any_dirty_documents()
            || !self.update_parked_idle()
            || self.any_pending_edits()
            || self.any_deferred_files()
            || self.update_external_busy()
            || self
                .saving
                .as_ref()
                .is_some_and(|s| s.failed() || s.pending())
        {
            self.update_restart = Some(State::Failed(t!(
                ctx,
                "Background shutdown failed. The update was not installed."
            )));
            return;
        }
        let Some(verified) = self.updates.take_verified_for_apply() else {
            self.update_restart = Some(State::Failed(t!(
                ctx,
                "PaintCraft could not finish installing the update. Close PaintCraft before trying again."
            )));
            return;
        };
        let Some(guard) = self.installation_lock.take() else {
            self.update_restart = Some(State::Failed(t!(
                ctx,
                "PaintCraft could not finish installing the update. Close PaintCraft before trying again."
            )));
            return;
        };
        // App keeps a strong slot even if thread creation fails or its body panics.
        let inputs = Arc::new(Mutex::new(ApplyInputs {
            guard: Some(guard),
            verified: Some(verified),
            result: None,
        }));
        let work = inputs.clone();
        let wake = ctx.clone();
        let thread = std::thread::Builder::new()
            .name("paintcraft-update-apply".into())
            .spawn(move || {
                let mut owned = match work.lock() {
                    Ok(owned) => owned,
                    Err(poison) => poison.into_inner(),
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let verified = owned
                        .verified
                        .take()
                        .ok_or(paintcraft_update::Error::ManualOnly)?;
                    let guard = owned
                        .guard
                        .as_mut()
                        .ok_or(paintcraft_update::Error::ManualOnly)?;
                    paintcraft_update::Updater::detect(env!("CARGO_PKG_VERSION"))?
                        .apply(verified, guard)
                        .map(|_| ())
                }))
                .unwrap_or_else(|_| {
                    Err(paintcraft_update::Error::Helper(
                        "update worker panicked".into(),
                    ))
                });
                owned.result = Some(result);
                drop(owned);
                wake.request_repaint();
            });
        match thread {
            Ok(thread) => {
                self.update_restart = Some(State::Applying(ApplyWorker { inputs, thread }))
            }
            Err(_) => {
                let mut owned = inputs.lock().unwrap_or_else(|poison| poison.into_inner());
                self.installation_lock = owned.guard.take();
                self.update_restart = Some(State::Failed(t!(
                    ctx,
                    "PaintCraft could not finish installing the update. Close PaintCraft before trying again."
                )));
            }
        }
    }
    fn poll_update_apply(&mut self, ctx: &egui::Context) {
        if !matches!(&self.update_restart, Some(State::Applying(worker)) if worker.thread.is_finished())
        {
            return;
        }
        let Some(State::Applying(worker)) = self.update_restart.take() else {
            return;
        };
        let joined = worker.thread.join().is_ok();
        let mut owned = match worker.inputs.lock() {
            Ok(owned) => owned,
            Err(poison) => poison.into_inner(),
        };
        self.installation_lock = owned.guard.take();
        let result = owned.result.take();
        self.update_restart = match result {
            Some(Ok(())) if joined => Some(State::Closing),
            Some(Err(error)) => Some(State::Failed(format!(
                "{}\n{error}",
                t!(
                    ctx,
                    "PaintCraft could not finish installing the update. Close PaintCraft before trying again."
                )
            ))),
            _ => Some(State::Failed(t!(
                ctx,
                "PaintCraft could not finish installing the update. Close PaintCraft before trying again."
            ))),
        };
    }
    pub(super) fn update_restart_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let first = ctx.current_pass_index() == 0;
        if matches!(self.update_restart, Some(State::Closing)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        match &self.update_restart {
            Some(State::Prompt { .. }) => {
                use paintcraft_ui::confirmation::{Choice, Prompt};
                match paintcraft_ui::confirmation::show(
                    &ctx,
                    egui::Id::unique("update-save-restart"),
                    Prompt {
                        title: "Save all documents and restart?",
                        message: "PaintCraft will save modified documents before installing the verified update.",
                        accept: "Save all and restart",
                        discard: None,
                    },
                ) {
                    Some(Choice::Accept) if first => self.begin_update_saves(&ctx),
                    Some(Choice::Cancel | Choice::Discard) if first => {
                        self.cancel_update_restart(&ctx)
                    }
                    _ => {}
                }
            }
            Some(State::Saving { .. }) => {
                ui.label(t!(&ctx, "Saving documents before update…"));
                if first {
                    self.pump_update_saves(ui);
                }
                if self.update_restart.is_some() && ui.button(t!(&ctx, "Cancel")).clicked() && first
                {
                    self.cancel_update_restart(&ctx);
                }
            }
            Some(State::Draining(_)) => {
                ui.label(t!(&ctx, "Stopping background work before update…"));
                if first {
                    let result = match self.update_restart.as_mut() {
                        Some(State::Draining(Some(drain))) => drain.poll(),
                        _ => None,
                    };
                    if let Some(result) = result {
                        match result {
                            Ok(()) => self.start_update_apply(&ctx),
                            Err(error) => {
                                self.update_restart = Some(State::Failed(format!(
                                    "{}\n{error}",
                                    t!(
                                        &ctx,
                                        "Background shutdown failed. The update was not installed."
                                    )
                                )))
                            }
                        }
                    } else if matches!(self.update_restart, Some(State::Draining(None))) {
                        self.drain_update_workers(&ctx);
                    }
                }
            }
            Some(State::Applying(_)) => {
                ui.label(t!(&ctx, "Installing verified update…"));
                if first {
                    self.poll_update_apply(&ctx);
                }
            }
            Some(State::Failed(message)) => {
                ui.label(message);
                if ui.button(t!(&ctx, "Close PaintCraft")).clicked() && first {
                    self.update_restart = Some(State::Closing);
                }
            }
            Some(State::Closing) | None => {}
        }
        ctx.request_repaint_after(Duration::from_millis(25));
    }
}

#[cfg(test)]
#[path = "update_restart/tests.rs"]
mod tests;
