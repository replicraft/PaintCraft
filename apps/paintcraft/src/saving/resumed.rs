//! Restart inspection owns no editor save request and cannot adopt a binding.
use super::*;
use paintcraft_formats::save_recovery::{self, reconciliation::ReconciledSavedAttempt};

#[derive(Clone, Debug)]
struct Reacquire {
    scope: Uuid,
    found: Found,
}
#[derive(Debug)]
pub(super) struct Request {
    session: Uuid,
    id: Uuid,
    scope: Uuid,
    found: Found,
}
impl Request {
    pub(super) fn owner(&self) -> Uuid {
        self.session
    }
}
#[derive(Debug)]
struct Report {
    scope: Uuid,
    found: Found,
    result: ReconciledSavedAttempt,
}
#[derive(Debug, Default)]
pub(super) struct State {
    dialog: Option<Reacquire>,
    active: Option<(Uuid, Uuid)>,
    report: Option<Report>,
}
impl State {
    pub(super) fn busy(&self) -> bool {
        self.dialog.is_some() || self.active.is_some()
    }
}
impl NativeSaving {
    pub(crate) fn cancel_inspection(&mut self) {
        self.inspection = State::default();
    }
    pub(super) fn inspection_busy(&self, found: &Found) -> bool {
        self.inspection
            .active
            .is_some_and(|(_, owner)| owner == found.session)
            || self
                .inspection
                .dialog
                .as_ref()
                .is_some_and(|pending| pending.found.session == found.session)
    }
    fn inspect_recovered(
        &mut self,
        scope: Uuid,
        found: Found,
        selected: PathBuf,
        mode: save::ReconcileMode,
    ) -> Result<()> {
        if self.inspection.active.is_some() {
            return Ok(());
        }
        let request = Request {
            session: self.session,
            id: Uuid::new_v4(),
            scope,
            found,
        };
        let source = request.found.clone();
        let context = self.context.clone();
        // Claim validation decodes a complete immutable snapshot. Use the same
        // exclusive provisional admission as native restore, not a metadata charge.
        let ticket = self.submit_io(
            1 << 30,
            move || {
                save_recovery::reconciliation::reconcile(&source, &selected, mode)
                    .map(Completion::Resumed)
            },
            move || context.request_repaint(),
        )?;
        self.inspection.active = Some((request.id, request.found.session));
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Resumed(Box::new(request)),
        });
        Ok(())
    }
}
impl App {
    pub(crate) fn locate_recovered_destination(&mut self, ctx: &egui::Context, found: Found) {
        let dialog_owner = self.active_tab_id();
        let Some(dialogs) = &mut self.dialogs else {
            self.editor.set_status(
                "Locating a saved destination requires the native desktop file picker.",
            );
            return;
        };
        if dialogs.pending() {
            return;
        }
        let Some(saving) = &mut self.saving else {
            return;
        };
        if saving.inspection_busy(&found) {
            return;
        }
        saving.inspection.dialog = Some(Reacquire {
            scope: self.recovery_scope,
            found,
        });
        let context = ctx.clone();
        // Stored diagnostic/display paths do not supply this read capability.
        self.dialog_owner = Some(dialog_owner);
        dialogs.request(Kind::InspectRecovery, None, move || {
            context.request_repaint()
        });
    }
    pub(crate) fn recovered_destination_selected(&mut self, selected: Option<PathBuf>) {
        let Some(saving) = &mut self.saving else {
            return;
        };
        let Some(pending) = saving.inspection.dialog.take() else {
            return;
        };
        if pending.scope != self.recovery_scope || !self.recoverable.contains(&pending.found) {
            return;
        }
        let Some(selected) = selected else {
            return;
        };
        if let Err(error) = saving.inspect_recovered(
            pending.scope,
            pending.found,
            selected,
            save::ReconcileMode::Inspect,
        ) {
            self.editor
                .set_status(format!("Destination inspection not queued: {error}"));
        }
    }
    pub(super) fn complete_resumed_inspection(
        &mut self,
        request: Request,
        result: Result<Completion>,
    ) {
        let saving = self.saving.as_mut().expect("native inspection");
        if saving.inspection.active != Some((request.id, request.found.session)) {
            return;
        }
        saving.inspection.active = None;
        if request.scope != self.recovery_scope || !self.recoverable.contains(&request.found) {
            return;
        }
        match result {
            Ok(Completion::Resumed(result)) => {
                saving.inspection.report = Some(Report {
                    scope: request.scope,
                    found: request.found,
                    result,
                });
                self.editor.set_status("Saved destination inspected. The current document and its save association are unchanged.");
            }
            Err(error) => {
                saving.inspection.report = None;
                self.editor
                    .set_status(format!("Saved destination inspection failed: {error}"));
            }
            _ => unreachable!("typed resumed inspection"),
        }
    }
    pub(crate) fn recovered_inspection_controls(&mut self, ui: &mut egui::Ui, found: &Found) {
        let Some(root) = &self.recovery_dir else {
            return;
        };
        if !save_recovery::is_saved_attempt(root, found) {
            return;
        }
        let Some(saving) = &self.saving else {
            return;
        };
        let busy = saving.inspection_busy(found) || saving.offer_busy(found);
        let report = saving
            .inspection
            .report
            .as_ref()
            .filter(|report| report.scope == self.recovery_scope && report.found == *found);
        let first_pass = ui.ctx().current_pass_index() == 0;
        let locate = ui
            .add_enabled(!busy, egui::Button::new("Locate saved destination…"))
            .clicked()
            && first_pass;
        let mut confirm = None;
        if let Some(report) = report {
            let confirmed = report
                .result
                .receipt
                .get("durability")
                .and_then(Value::as_str)
                == Some("confirmed");
            ui.label(match report.result.observation {
                save::Observation::Staged if confirmed => "The captured content is present and its durability was confirmed. Recovery remains available.",
                save::Observation::Staged => "The captured content is present. This alone does not prove a completed durable save.",
                save::Observation::Original => "The content observed before the save is present.",
                save::Observation::Other => "The destination contains different content.",
                save::Observation::Missing => "The destination is missing.",
                save::Observation::Unreadable => "The destination could not be verified.",
            });
            if ui
                .add_enabled(
                    !busy && !confirmed,
                    egui::Button::new("Confirm destination durability"),
                )
                .clicked()
                && first_pass
            {
                confirm = Some(report.result.destination.clone());
            }
        }
        if busy {
            ui.spinner();
        }
        if locate {
            self.locate_recovered_destination(ui.ctx(), found.clone());
        }
        if let Some(selected) = confirm
            && let Some(saving) = &mut self.saving
            && let Err(error) = saving.inspect_recovered(
                self.recovery_scope,
                found.clone(),
                selected,
                save::ReconcileMode::ConfirmDurability,
            )
        {
            self.editor
                .set_status(format!("Durability confirmation not queued: {error}"));
        }
    }
}

#[cfg(test)]
mod tests;
