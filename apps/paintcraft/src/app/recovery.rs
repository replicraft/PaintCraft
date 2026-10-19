//! Desktop recovery integration.

use super::*;

impl App {
    /// Protect work with crash-recovery checkpoints in `dir`, and offer whatever an
    /// earlier crashed session left behind.
    pub fn with_recovery(mut self, dir: PathBuf) -> App {
        // This is startup configuration, not a live store-migration API. Retain
        // the active owner until a separately implemented durable handoff exists.
        if self.editor.recovery_handle().is_some() {
            if self.recovery_dir.as_ref() != Some(&dir) {
                self.editor.set_status("The recovery folder cannot be changed while it is active. Existing recovery data has been retained.");
            }
            return self;
        }
        self.recovery_scope = Uuid::new_v4();
        self.recoverable.clear();
        if self.saving.is_some() {
            self.recovery_needs_scan = true;
        } else {
            let report = Recovery::scan_report(&dir);
            self.recoverable = report.found;
            if !report.issues.is_empty() {
                self.editor.set_status(
                    report
                        .issues
                        .into_iter()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("; "),
                );
            }
        }
        self.recovery_dir = Some(dir);
        self.recovery_needs_init = true;
        self.attach_recovery();
        self
    }

    pub(super) fn attach_recovery(&mut self) {
        let Some(dir) = &self.recovery_dir else {
            return;
        };
        if let Some(saving) = &mut self.saving {
            match saving.initialize_recovery(
                self.recovery_scope,
                dir.clone(),
                self.recovery_needs_scan,
            ) {
                Ok(()) => {
                    self.recovery_needs_init = false;
                    self.editor.set_status("Checking crash recovery…");
                }
                Err(error) if error.code == paintcraft_core::types::ErrorCode::ResourceLimit => {
                    self.recovery_needs_init = true;
                    self.editor
                        .set_status("Waiting for recovery initialization capacity…");
                }
                Err(error) => {
                    self.recovery_needs_init = false;
                    self.editor
                        .set_status(format!("Crash recovery unavailable: {error}"));
                }
            }
        } else {
            self.recovery_needs_init = false;
            match Recovery::start(dir) {
                Ok(recovery) => self.editor.attach_recovery(recovery),
                Err(error) => self
                    .editor
                    .set_status(format!("Crash recovery unavailable: {error}")),
            }
        }
    }

    pub(super) fn recover_requested(&mut self, item: Found) {
        self.editor.commit_text();
        self.editor.run_pending();
        if self.editor.is_dirty() {
            self.confirm = Some(Confirm::DiscardForRecovery(item));
        } else {
            self.do_recover(item);
        }
    }

    pub(super) fn do_recover(&mut self, item: Found) {
        self.editor.cancel_gesture();
        self.editor.commit_text();
        self.editor.run_pending();
        let drafts = saving::restoring::Drafts::capture(self);
        if let Some(saving) = &mut self.saving {
            match saving.request_restore(&self.editor, self.recovery_scope, item, drafts) {
                Ok(()) => self.editor.set_status("Preparing recovered document…"),
                Err(error) => self.editor.set_status(format!("Cannot recover: {error}")),
            }
        } else {
            match self.recover_found(&item) {
                Ok(()) => self
                    .recoverable
                    .retain(|found| found.session != item.session),
                Err(error) => self.editor.set_status(format!("Cannot recover: {error}")),
            }
        }
    }

    pub(super) fn recover_found(&mut self, item: &Found) -> paintcraft_core::types::Result<()> {
        let claimed = Recovery::claim(item)?;
        self.prepare_editor_replacement()?;
        self.publish_editor(Editor::recovered_document(claimed.document));
        self.recovery_source = Some(saving::restoring::Handoff::new(claimed.lease));
        self.bound = None;
        if let Some(original) = &item.original {
            self.path_text.clone_from(original);
        }
        if self.editor.autosave() {
            self.protect_recovery_source();
            self.pump_recovery_handoff();
        } else {
            self.editor.set_status("Recovered; the original recovery copy is retained because a new checkpoint could not be confirmed.");
        }
        Ok(())
    }

    pub(super) fn recovery_dialog(&mut self, ui: &mut egui::Ui) {
        if self.recoverable.is_empty() {
            return;
        }
        let ctx = ui.ctx().clone();
        let item = self.recoverable[0].clone();
        let name = item
            .original
            .clone()
            .unwrap_or_else(|| "an unsaved document".to_owned());
        egui::Window::new("Recover unsaved work?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label(format!(
                    "PaintCraft found unsaved work from an earlier session: {name}."
                ));
                self.recovered_inspection_controls(ui, &item);
                let restoring = self
                    .saving
                    .as_ref()
                    .is_some_and(|saving| saving.offer_busy(&item));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.confirm.is_none() && !restoring,
                            egui::Button::new("Recover"),
                        )
                        .clicked()
                    {
                        self.recover_requested(item.clone());
                    }
                    if ui
                        .add_enabled(!restoring, egui::Button::new("Discard"))
                        .clicked()
                        && let Some(dir) = &self.recovery_dir
                    {
                        if let Some(saving) = &mut self.saving {
                            match saving.discard_offer(self.recovery_scope, &item) {
                                Ok(()) => self.editor.set_status("Discarding recovery copy…"),
                                Err(error) => self
                                    .editor
                                    .set_status(format!("Cannot discard recovery: {error}")),
                            }
                        } else {
                            let source = item.checkpoint.parent().unwrap_or(dir);
                            match Recovery::discard(source, item.session) {
                                Ok(()) => {
                                    self.recoverable.remove(0);
                                }
                                Err(error) => self
                                    .editor
                                    .set_status(format!("Cannot discard recovery: {error}")),
                            }
                        }
                    }
                    if ui.button("Ask later").clicked() {
                        if let Some(saving) = &mut self.saving {
                            saving.cancel_restore();
                            saving.cancel_inspection();
                        }
                        self.recoverable.clear();
                    }
                });
            });
    }
}
