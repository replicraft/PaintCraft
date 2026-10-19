//! Retain captured incomplete saves and reconcile them on the existing I/O FIFO.
use super::*;

const ATTEMPTS_MAX: usize = 16;
const CONTROL_BYTES: u64 = 64 * 1024;
const CONTROL_RESERVE: u64 = 1024 * 1024;

#[derive(Debug)]
pub(super) struct Attempt {
    ticket: Ticket,
    request: SaveRequest,
    outcome: Option<save::Outcome>,
    message: String,
    checking: bool,
}

impl NativeSaving {
    pub(super) fn admit_save(&self, charge: u64) -> Result<()> {
        let pending = self
            .pending
            .iter()
            .filter(|p| matches!(p.kind, PendingKind::Save(_)))
            .count();
        if self.attempts.len() + pending >= ATTEMPTS_MAX {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "resolve incomplete saves before requesting more saves",
            ));
        }
        let reserved = charge.checked_add(CONTROL_RESERVE).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "save control reservation overflows",
            )
        })?;
        self.admit(1, reserved)
    }
    pub(super) fn admit(&self, jobs: usize, bytes: u64) -> Result<()> {
        let retained = self
            .attempts
            .iter()
            .try_fold(bytes, |sum, attempt| {
                sum.checked_add(attempt.request.charge)
            })
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "incomplete save retention overflows",
                )
            })?;
        self.io.admit(jobs, retained)
    }
    pub(super) fn submit_io(
        &mut self,
        bytes: u64,
        work: impl FnOnce() -> Result<Completion> + Send + 'static,
        wake: impl FnOnce() + Send + 'static,
    ) -> Result<Ticket> {
        self.admit(1, bytes)?;
        self.io.submit(bytes, work, wake)
    }
    fn remember_receipt(&mut self, ticket: Ticket, request: &SaveRequest, outcome: &save::Outcome) {
        if ticket.sequence() >= self.receipt_sequence {
            self.receipt_sequence = ticket.sequence();
            self.last_receipt = Some(outcome.receipt_stamped(
                &save::Context {
                    operation: request.operation,
                    source_state: request.source,
                    source_sha256: None,
                    losses_acknowledged: vec![],
                    destination: &request.path,
                },
                ticket.sequence(),
                request.generation,
            ));
        }
    }
    fn request_reconciliation(&mut self, sequence: u64, mode: save::ReconcileMode) -> Result<()> {
        let attempt = self
            .attempts
            .iter()
            .find(|a| a.ticket.sequence() == sequence)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::TargetInvalid,
                    "incomplete save is no longer available",
                )
            })?;
        if attempt.checking {
            return Ok(());
        }
        let outcome = attempt
            .outcome
            .as_ref()
            .filter(|o| o.is_reconcilable())
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::TargetInvalid,
                    "no captured replacement evidence to inspect",
                )
            })?
            .clone();
        let context = self.context.clone();
        let ticket = self.submit_io(
            CONTROL_BYTES,
            move || outcome.reconcile(mode).map(Completion::Reconciled),
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Reconcile { sequence },
        });
        self.attempts
            .iter_mut()
            .find(|a| a.ticket.sequence() == sequence)
            .expect("retained attempt")
            .checking = true;
        Ok(())
    }
}

impl App {
    pub(super) fn complete_save(
        &mut self,
        ticket: Ticket,
        request: SaveRequest,
        result: Result<Completion>,
    ) {
        let saving = self.saving.as_mut().expect("native I/O");
        let outcome = match result {
            Ok(Completion::File(outcome)) => outcome,
            Err(error) => {
                // Worker failure cannot establish how far a replacement got.
                self.editor.set_status(format!(
                    "Save outcome unconfirmed for {}: {error}",
                    request.path.display()
                ));
                saving.attempts.push(Attempt {
                    ticket,
                    request,
                    outcome: None,
                    message: error.to_string(),
                    checking: false,
                });
                return;
            }
            _ => unreachable!("typed save completion"),
        };
        saving.remember_receipt(ticket, &request, &outcome);
        let success = outcome.is_success();
        if success {
            self.adopt_saved_attempt(ticket, &request, &outcome);
        } else {
            self.editor.set_status(format!(
                "{} was not saved safely: {}",
                request.path.display(),
                outcome.reasons.join("; ")
            ));
            let saving = self.saving.as_mut().expect("native I/O");
            saving.attempts.push(Attempt {
                ticket,
                request,
                message: outcome.reasons.join("; "),
                outcome: Some(outcome),
                checking: false,
            });
        }
    }
    fn adopt_saved_attempt(
        &mut self,
        ticket: Ticket,
        request: &SaveRequest,
        outcome: &save::Outcome,
    ) {
        let saving = self.saving.as_mut().expect("native I/O");
        if request.session != saving.session
            || request.generation != saving.generation
            || ticket.sequence() < saving.adopted_sequence
            || !outcome.is_success()
        {
            return;
        }
        // Bind the originally captured namespace even if the dialog alias changed.
        let path = outcome
            .resolved_destination()
            .unwrap_or(&request.path)
            .to_path_buf();
        saving.adopted_sequence = ticket.sequence();
        self.editor.set_path(path.clone());
        self.path_text = path.to_string_lossy().into_owned();
        self.bound = Some(path.clone());
        self.editor.acknowledge_saved_state(request.source);
        if let Some(source) = &mut self.recovery_source {
            source.protect();
        }
        self.editor.set_status(format!(
            "Saved {}{}",
            path.display(),
            if self.editor.is_dirty() {
                "; newer edits remain unsaved"
            } else {
                ""
            }
        ));
        if !self.editor.is_dirty()
            && let Err(error) = saving.clear_recovery(&mut self.editor)
        {
            self.editor.set_status(format!(
                "Saved {}; recovery cleanup not queued: {error}",
                path.display()
            ));
        }
    }
    pub(super) fn complete_reconciliation(&mut self, sequence: u64, result: Result<Completion>) {
        let saving = self.saving.as_mut().expect("native I/O");
        let Some(index) = saving
            .attempts
            .iter()
            .position(|a| a.ticket.sequence() == sequence)
        else {
            return;
        };
        let mut attempt = saving.attempts.remove(index);
        attempt.checking = false;
        match result {
            Ok(Completion::Reconciled(checked)) => {
                saving.remember_receipt(attempt.ticket, &attempt.request, &checked.outcome);
                if checked.outcome.is_success() {
                    self.adopt_saved_attempt(attempt.ticket, &attempt.request, &checked.outcome);
                } else {
                    attempt.message = match checked.observation {
                            save::Observation::Staged =>
                                "Expected content is present; durability remains unconfirmed.",
                            save::Observation::Original =>
                                "The original content is present. The save remains unresolved.",
                            save::Observation::Other =>
                                "The destination contains different content. The save remains unresolved.",
                            save::Observation::Missing =>
                                "The destination is missing. The save remains unresolved.",
                            save::Observation::Unreadable =>
                                "The destination could not be verified. The save remains unresolved.",
                        }.to_owned();
                    attempt.outcome = Some(checked.outcome);
                    self.saving
                        .as_mut()
                        .expect("native I/O")
                        .attempts
                        .insert(index, attempt);
                }
            }
            Err(error) => {
                attempt.message = format!("Inspection failed: {error}");
                saving.attempts.insert(index, attempt);
            }
            _ => unreachable!("typed reconciliation completion"),
        }
    }
    pub(crate) fn saved_snapshot_requested(&mut self, sequence: u64) {
        self.editor.commit_text();
        self.editor.run_pending();
        let layers = &self.editor.document().layers;
        let name_draft = self
            .name_edit
            .as_ref()
            .is_some_and(|(id, name)| layers.get(id).is_some_and(|layer| layer.name != *name));
        let opacity_draft = self.opacity_edit.is_some_and(|(id, opacity)| {
            layers
                .get(&id)
                .is_some_and(|layer| layer.opacity != opacity)
        });
        if self.editor.is_dirty()
            || self.editor.gesture_in_progress()
            || name_draft
            || opacity_draft
        {
            self.confirm = Some(Confirm::DiscardForSaveSnapshot(sequence));
        } else {
            self.open_saved_snapshot(sequence);
        }
    }

    pub(crate) fn open_saved_snapshot(&mut self, sequence: u64) {
        self.editor.cancel_gesture();
        let snapshot = self.saving.as_ref().and_then(|saving| {
            saving
                .attempts
                .iter()
                .find(|a| a.ticket.sequence() == sequence)
                .map(|a| a.request.snapshot.clone())
        });
        let Some(snapshot) = snapshot else {
            self.editor
                .set_status("That captured save snapshot is no longer available.");
            return;
        };
        match self.install(Editor::recovered_document((*snapshot).clone())) {
            Ok(()) => {
                self.bound = None;
                self.path_text = "recovered-save.pcnd".into();
                self.name_edit = None;
                self.opacity_edit = None;
                self.editor.set_status("Opened the captured save snapshot as unsaved work. Choose Save As to keep a copy.");
            }
            Err(error) => self
                .editor
                .set_status(format!("Cannot open captured snapshot: {error}")),
        }
    }

    pub(crate) fn forget_incomplete_save(&mut self, sequence: u64) {
        let Some(saving) = &mut self.saving else {
            return;
        };
        let Some(index) = saving
            .attempts
            .iter()
            .position(|a| a.ticket.sequence() == sequence)
        else {
            return;
        };
        if saving.attempts[index].checking {
            self.editor
                .set_status("Wait for inspection to finish before forgetting this save.");
            return;
        }
        saving.attempts.remove(index);
        self.editor
            .set_status("Captured save snapshot forgotten. No destination was changed.");
    }

    pub(crate) fn incomplete_save_panel(&mut self, ui: &mut egui::Ui) {
        let Some(saving) = &self.saving else {
            return;
        };
        if saving.attempts.is_empty() {
            return;
        }
        let mut action = None;
        let mut recover = None;
        let mut forget = None;
        let first_pass = ui.ctx().current_pass_index() == 0;
        egui::CollapsingHeader::new(format!("Incomplete saves ({})", saving.attempts.len()))
            .id_salt("incomplete-save-attempts")
            .show(ui, |ui| {
                ui.label("Captured snapshots are kept while this window is open. Inspection does not repeat a save.");
                egui::ScrollArea::vertical()
                    .id_salt("incomplete-save-list")
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for attempt in &saving.attempts {
                            ui.push_id(attempt.ticket.sequence(), |ui| {
                                ui.label(attempt.request.path.to_string_lossy());
                                ui.label(&attempt.message);
                                let enabled = !attempt.checking
                                    && attempt.outcome.as_ref().is_some_and(save::Outcome::is_reconcilable);
                                for (label, mode) in [
                                    ("Inspect destination", save::ReconcileMode::Inspect),
                                    ("Confirm durability", save::ReconcileMode::ConfirmDurability),
                                ] {
                                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() && first_pass {
                                        action = Some((attempt.ticket.sequence(), mode));
                                    }
                                }
                                if ui.button("Open captured snapshot").clicked() && first_pass {
                                    recover = Some(attempt.ticket.sequence());
                                }
                                if ui.add_enabled(!attempt.checking, egui::Button::new("Forget captured snapshot")).clicked() && first_pass {
                                    forget = Some(attempt.ticket.sequence());
                                }
                                if attempt.checking { ui.spinner(); }
                                ui.separator();
                            });
                        }
                    });
            });
        if let Some(sequence) = forget {
            self.confirm = Some(Confirm::ForgetSaveSnapshot(sequence));
        }
        if let Some(sequence) = recover {
            self.saved_snapshot_requested(sequence);
        }
        if let Some((sequence, mode)) = action
            && let Some(saving) = &mut self.saving
            && let Err(error) = saving.request_reconciliation(sequence, mode)
        {
            self.editor
                .set_status(format!("Inspection not queued: {error}"));
        }
    }
}

#[cfg(test)]
mod tests;
