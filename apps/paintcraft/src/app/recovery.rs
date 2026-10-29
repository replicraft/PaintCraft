//! Desktop recovery integration.

use super::*;
use paintcraft_ui::t;

impl App {
    /// Protect work with crash-recovery checkpoints in `dir`, and offer whatever an
    /// earlier crashed session left behind.
    pub fn with_recovery(mut self, dir: PathBuf) -> App {
        // This is startup configuration, not a live store-migration API. Retain
        // the active owner until a separately implemented durable handoff exists.
        if self.has_live_recovery() {
            if self.recovery_dir.as_ref() != Some(&dir) {
                self.editor.set_status("The recovery folder cannot be changed while it is active. Existing recovery data has been retained.");
            }
            return self;
        }
        self.recovery_scope = Uuid::new_v4();
        if self.saving.is_some() {
            self.recovery_needs_scan = true;
        } else {
            let budget = self.source_metadata_budget();
            let report = Recovery::scan_report_with_metadata_admission_checked(
                &dir,
                paintcraft_formats::pcnd::MetadataAdmission {
                    bytes: paintcraft_document::source_metadata::BYTES_MAX,
                    budget: &budget,
                },
                &|| Ok(()),
            );
            self.apply_recovery_discovery(report);
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
        let budget = self.source_metadata_budget();
        if let Some(saving) = &mut self.saving {
            match saving.initialize_recovery(
                self.recovery_scope,
                dir.clone(),
                self.recovery_needs_scan,
                budget,
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
        // Inspect private text without finalizing it: failed preparation must not
        // turn a user's draft into a document/history transaction.
        if self.experience() == Experience::Classic
            && (self.editor.is_dirty()
                || self
                    .editor
                    .canvas_text_content()
                    .is_some_and(|text| !text.is_empty()))
        {
            self.confirm = Some(Confirm::DiscardForRecovery(item));
        } else {
            self.do_recover(item);
        }
    }

    pub(super) fn do_recover(&mut self, item: Found) {
        self.editor.run_pending();
        let drafts = saving::restoring::Drafts::capture(self);
        let budget = self.source_metadata_budget();
        if let Some(saving) = &mut self.saving {
            match saving.request_restore(&self.editor, self.recovery_scope, item, drafts, budget) {
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
        let budget = self.source_metadata_budget();
        let claimed = Recovery::claim_with_metadata_admission_checked(
            item,
            paintcraft_formats::pcnd::MetadataAdmission {
                bytes: paintcraft_document::source_metadata::BYTES_MAX,
                budget: &budget,
            },
            &|| Ok(()),
        )?;
        if self.experience() == Experience::Studio || self.tabs.is_empty() {
            self.check_tab_capacity()?;
        }
        // install resolves the previous tab's input only after source and capacity
        // admission; Classic replaces only after its explicit discard decision.
        self.install(Editor::recovered_document(claimed.document))?;
        self.forget_recovery_offer(item.session);
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

    pub(crate) fn apply_recovery_discovery(
        &mut self,
        result: paintcraft_core::types::Result<paintcraft_formats::recovery::ScanReport>,
    ) {
        apply_discovery(
            &mut self.editor,
            &mut self.recoverable,
            &mut self.recovery_needs_scan,
            &self.context,
            result,
        );
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
            .unwrap_or_else(|| t!(&ctx, "an unsaved document"));
        egui::Window::new(t!(&ctx, "Recover unsaved work?"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(&ctx, |ui| {
                ui.label(t!(
                    ui.ctx(),
                    "PaintCraft found unsaved work from an earlier session: { $name }.",
                    &[("name", name.clone())],
                ));
                self.recovered_inspection_controls(ui, &item);
                let restoring = self.recovery_offer_busy(&item);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.confirm.is_none() && !restoring,
                            egui::Button::new(t!(ui.ctx(), "Recover")),
                        )
                        .clicked()
                    {
                        self.recover_requested(item.clone());
                    }
                    if ui
                        .add_enabled(!restoring, egui::Button::new(t!(ui.ctx(), "Discard")))
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
                    if ui.button(t!(ui.ctx(), "Ask later")).clicked() {
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

#[cfg(test)]
#[path = "recovery/metadata_tests.rs"]
mod metadata_tests;

pub(crate) fn apply_discovery(
    editor: &mut Editor,
    recoverable: &mut Vec<Found>,
    needs_scan: &mut bool,
    ctx: &egui::Context,
    result: paintcraft_core::types::Result<paintcraft_formats::recovery::ScanReport>,
) {
    match result {
        Ok(report) => {
            *needs_scan = false;
            *recoverable = report.found;
            if report.unsupported_sessions != 0 {
                editor.set_status(t!(
                    ctx,
                    "Some recovery copies need a newer PaintCraft version. Those copies were kept."
                ));
            } else if !report.issues.is_empty() {
                editor.set_status(
                    report
                        .issues
                        .into_iter()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("; "),
                );
            }
        }
        Err(error) => {
            // No per-frame reinitialization: the current store remains useful.
            // Existing explicit owner/store initialization can retry discovery.
            *needs_scan = true;
            editor.set_status(match error.code {
                    paintcraft_core::types::ErrorCode::ResourceLimit=>t!(ctx,"Crash recovery discovery reached a resource limit. Existing recovery copies were kept."),
                    paintcraft_core::types::ErrorCode::OperationCancelled=>t!(ctx,"Crash recovery discovery was cancelled. Existing recovery copies were kept."),
                    _=>t!(ctx,"Crash recovery discovery failed. Existing recovery copies were kept."),
                });
        }
    }
}
