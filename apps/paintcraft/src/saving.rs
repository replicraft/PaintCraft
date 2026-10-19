//! Native I/O coordinator. Saves, periodic checkpoints and recovery cleanup share
//! one bounded FIFO. Only the UI coordinator adopts results into editor state.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value};
use paintcraft_formats::recovery::{Checkpoint, RecoveryToken};
use paintcraft_runtime::io::{Limits, OrderedIo, Ticket};

#[derive(Debug)]
struct SaveRequest {
    session: Uuid,
    generation: u64,
    source: Uuid,
    path: PathBuf,
    operation: &'static str,
}
#[derive(Debug)]
enum PendingKind {
    Save(SaveRequest),
    Checkpoint {
        session: Uuid,
        token: RecoveryToken,
        key: (Uuid, u64),
    },
    Initialize {
        session: Uuid,
        scope: Uuid,
    },
    Cleanup,
}
#[derive(Debug)]
struct Pending {
    ticket: Ticket,
    kind: PendingKind,
}
#[derive(Debug)]
enum Completion {
    File(save::Outcome),
    Checkpoint(Checkpoint),
    Started {
        report: Option<paintcraft_formats::recovery::ScanReport>,
        recovery: Result<Arc<Recovery>>,
    },
    Cleanup,
}
#[derive(Debug)]
pub(super) struct NativeSaving {
    io: OrderedIo<Completion>,
    context: egui::Context,
    session: Uuid,
    generation: u64,
    pending: Vec<Pending>,
    failed: bool,
    last_receipt: Option<Value>,
}
fn snapshot_charge(document: &paintcraft_document::Document) -> Result<u64> {
    // Provisional independent snapshot payload charge, not allocator/RSS accounting.
    document
        .layers
        .values()
        .try_fold(
            (64u64 << 20) + document.title.as_ref().map_or(0, |s| s.len() as u64),
            |bytes, layer| {
                bytes
                    .checked_add(
                        (layer.raster.resident_tiles() as u64).checked_mul(
                            paintcraft_raster::RasterLayer::TILE_BYTES as u64 + 8192,
                        )?,
                    )?
                    .checked_add(layer.name.len() as u64 + 4096)
            },
        )
        .ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "I/O snapshot retention charge overflows",
            )
        })
}
impl NativeSaving {
    pub(super) fn new(context: egui::Context) -> Result<Self> {
        Ok(Self {
            io: OrderedIo::new(Limits {
                pending_jobs: 4,
                retained_bytes: 1 << 30,
            })?,
            context,
            session: Uuid::new_v4(),
            generation: 0,
            pending: Vec::new(),
            failed: false,
            last_receipt: None,
        })
    }
    pub(super) fn new_session(&mut self) {
        self.session = Uuid::new_v4();
        self.generation = 0;
    }
    pub(super) fn failed(&self) -> bool {
        self.failed
    }
    pub(super) fn pending(&self) -> bool {
        self.io.pending_jobs() != 0
    }
    pub(super) fn submit(
        &mut self,
        editor: &Editor,
        path: PathBuf,
        bound: Option<&Path>,
        ctx: egui::Context,
    ) -> Result<()> {
        let operation = if bound == Some(path.as_path()) {
            "save"
        } else {
            "save-as"
        };
        let generation = if operation == "save-as" {
            self.generation.checked_add(1).ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "save binding generation exhausted",
                )
            })?
        } else {
            self.generation
        };
        let document = editor.document_snapshot();
        let source = document.state_id;
        let charge = snapshot_charge(&document)?;
        let destination = path.clone();
        let ticket = self.io.submit(
            charge,
            move || {
                Ok(Completion::File(save::save_with(
                    &destination,
                    true,
                    save::STAGED_FILE_BYTES_MAX,
                    |sink| paintcraft_formats::pcnd::write_to(&document, sink),
                )))
            },
            move || ctx.request_repaint(),
        )?;
        self.generation = generation;
        if self.pending.is_empty() {
            self.failed = false;
        }
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Save(SaveRequest {
                session: self.session,
                generation,
                source,
                path,
                operation,
            }),
        });
        Ok(())
    }
    pub(super) fn checkpoint(&mut self, editor: &Editor) -> Result<bool> {
        let Some((key, recovery)) = editor.recovery_candidate() else {
            return Ok(false);
        };
        let token = recovery.token();
        if self.pending.iter().any(|pending| matches!(pending.kind, PendingKind::Checkpoint { token: existing, .. } if existing == token)) { return Ok(false); }
        let document = editor.document_snapshot();
        let charge = snapshot_charge(&document)?;
        let original = editor.path().map(Path::to_owned);
        let context = self.context.clone();
        let ticket = self.io.submit(
            charge,
            move || {
                recovery
                    .checkpoint_document_at(token, &document, original.as_deref())
                    .map(Completion::Checkpoint)
            },
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Checkpoint {
                session: self.session,
                token,
                key,
            },
        });
        Ok(true)
    }
    fn poll_completion(&mut self) -> Option<(Ticket, PendingKind, Result<Completion>)> {
        let (ticket, result) = self.io.poll()?;
        let index = self
            .pending
            .iter()
            .position(|p| p.ticket == ticket)
            .expect("admitted I/O job");
        Some((ticket, self.pending.remove(index).kind, result))
    }
    pub(super) fn initialize_recovery(
        &mut self,
        scope: Uuid,
        directory: PathBuf,
        discover: bool,
    ) -> Result<()> {
        if self.pending.iter().any(|p| matches!(p.kind, PendingKind::Initialize { session, scope: pending_scope } if session == self.session && scope == pending_scope)) { return Ok(()); }
        // Discovery runs the native parser. This exclusive estimate prevents other
        // I/O payload jobs from overlapping it; it is not an RSS qualification.
        let charge = if discover { 1 << 30 } else { 64 * 1024 };
        let context = self.context.clone();
        let ticket = self.io.submit(
            charge,
            move || {
                let report = discover.then(|| Recovery::scan_report(&directory));
                let recovery = Recovery::start(&directory).map(Arc::new);
                Ok(Completion::Started { report, recovery })
            },
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Initialize {
                session: self.session,
                scope,
            },
        });
        Ok(())
    }
    fn retire_recovery(&mut self, recovery: Arc<Recovery>) -> Result<()> {
        self.io.admit(1, 64 * 1024)?;
        let token = recovery.invalidate()?;
        let context = self.context.clone();
        let ticket = self.io.submit(
            64 * 1024,
            move || {
                recovery.clear_at(token)?;
                Ok(Completion::Cleanup)
            },
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Cleanup,
        });
        Ok(())
    }
    pub(super) fn clear_recovery(&mut self, editor: &mut Editor) -> Result<()> {
        let Some(recovery) = editor.recovery_handle() else {
            editor.forget_checkpoint();
            return Ok(());
        };
        // Capacity is checked before invalidation, so an admission failure leaves
        // existing recovery work and the current editor intact.
        self.io.admit(1, 64 * 1024)?;
        let token = recovery.invalidate()?;
        let context = self.context.clone();
        let ticket = self.io.submit(
            64 * 1024,
            move || {
                recovery.clear_at(token)?;
                Ok(Completion::Cleanup)
            },
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Cleanup,
        });
        editor.forget_checkpoint();
        Ok(())
    }
}
impl App {
    pub(super) fn clean_recovery_before_close(&mut self) -> Result<bool> {
        if self.editor.is_dirty() || self.editor.checkpoint_info().is_none() {
            return Ok(false);
        }
        if let Some(saving) = &mut self.saving {
            saving.clear_recovery(&mut self.editor)?;
            Ok(true)
        } else {
            self.editor.discard_checkpoint()?;
            Ok(false)
        }
    }

    pub(super) fn close_after_requested_saves(&mut self, ctx: &egui::Context) {
        if self.saving.as_ref().is_some_and(NativeSaving::pending) {
            self.close_after_saves = true;
            self.editor
                .set_status("Waiting for requested saves and recovery before closing…");
        } else if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            match self.clean_recovery_before_close() {
                Ok(true) => self.close_after_saves = true,
                Ok(false) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(error) => {
                    self.allow_close = false;
                    self.editor
                        .set_status(format!("Recovery cleanup before close failed: {error}"));
                    self.confirm = Some(Confirm::QuitUnsaved);
                }
            }
        }
    }
    pub(super) fn poll_native_saves(&mut self, ctx: &egui::Context) {
        if self.saving.is_none() {
            return;
        }
        while let Some((ticket, kind, result)) =
            self.saving.as_mut().and_then(NativeSaving::poll_completion)
        {
            let saving = self.saving.as_mut().expect("native I/O");
            match kind {
                PendingKind::Save(request) => {
                    let outcome = match result {
                        Ok(Completion::File(outcome)) => outcome,
                        Err(error) => {
                            saving.failed = true;
                            self.editor.set_status(format!(
                                "Save outcome unconfirmed for {}: {error}",
                                request.path.display()
                            ));
                            continue;
                        }
                        _ => unreachable!("typed save completion"),
                    };
                    let success = outcome.is_success();
                    saving.failed |= !success;
                    saving.last_receipt = Some(outcome.receipt_stamped(
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
                    if request.session == saving.session
                        && request.generation == saving.generation
                        && success
                    {
                        self.editor.set_path(request.path.clone());
                        self.path_text = request.path.to_string_lossy().into_owned();
                        self.bound = Some(request.path.clone());
                        self.editor.acknowledge_saved_state(request.source);
                        self.editor.set_status(format!(
                            "Saved {}{}",
                            request.path.display(),
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
                                request.path.display()
                            ));
                        }
                    }
                    if !success {
                        self.editor.set_status(format!(
                            "{} was not saved safely: {}",
                            request.path.display(),
                            outcome.reasons.join("; ")
                        ));
                    }
                }
                PendingKind::Checkpoint {
                    session,
                    token,
                    key,
                } => {
                    if session != saving.session
                        || self
                            .editor
                            .recovery_handle()
                            .is_none_or(|r| !r.is_current(token))
                    {
                        continue;
                    }
                    match result {
                        Ok(Completion::Checkpoint(info)) => {
                            self.editor.accept_checkpoint(token, key, info)
                        }
                        Err(error) if error.code == ErrorCode::OperationCancelled => {}
                        Err(error) => {
                            saving.failed = true;
                            self.editor.set_status(format!("Autosave failed: {error}"));
                        }
                        _ => unreachable!("typed checkpoint completion"),
                    }
                }
                PendingKind::Initialize { session, scope } => match result {
                    Ok(Completion::Started { report, recovery }) => {
                        if scope == self.recovery_scope {
                            if let Some(report) = report
                                && self.recovery_needs_scan
                            {
                                self.recovery_needs_scan = false;
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
                            if session == saving.session {
                                match recovery {
                                    Ok(recovery) if self.editor.recovery_handle().is_none() => {
                                        self.editor.attach_shared_recovery(recovery);
                                        if self.editor.status() == "Checking crash recovery…" {
                                            self.editor.set_status("Crash recovery ready");
                                        }
                                    }
                                    Ok(recovery) => {
                                        if let Err(error) = saving.retire_recovery(recovery) {
                                            self.editor.set_status(format!(
                                                "Unused recovery cleanup failed: {error}"
                                            ));
                                            saving.failed = true;
                                        }
                                    }
                                    Err(error) => self
                                        .editor
                                        .set_status(format!("Crash recovery unavailable: {error}")),
                                }
                                continue;
                            }
                        }
                        if let Ok(recovery) = recovery
                            && let Err(error) = saving.retire_recovery(recovery)
                        {
                            self.editor
                                .set_status(format!("Unused recovery cleanup failed: {error}"));
                            saving.failed = true;
                        }
                    }
                    Err(error) => self
                        .editor
                        .set_status(format!("Recovery initialization failed: {error}")),
                    _ => unreachable!("typed initialization completion"),
                },
                PendingKind::Cleanup => match result {
                    Ok(Completion::Cleanup) => {}
                    Err(error) if error.code == ErrorCode::OperationCancelled => {}
                    Err(error) => {
                        saving.failed = true;
                        self.editor
                            .set_status(format!("Recovery cleanup failed: {error}"));
                    }
                    _ => unreachable!("typed cleanup completion"),
                },
            }
        }
        let saving = self.saving.as_mut().expect("native I/O");
        if self.close_after_saves && !saving.pending() {
            self.close_after_saves = false;
            if saving.failed {
                self.allow_close = false;
                self.editor.set_status("A requested save or recovery operation failed or remains unconfirmed. Review the document before closing.");
            } else if !self.editor.is_dirty() && self.editor.checkpoint_info().is_some() {
                match saving.clear_recovery(&mut self.editor) {
                    Ok(()) => self.close_after_saves = true,
                    Err(error) => {
                        saving.failed = true;
                        self.allow_close = false;
                        self.editor
                            .set_status(format!("Recovery cleanup before close failed: {error}"));
                    }
                }
            } else if self.editor.is_dirty() && !self.allow_close {
                self.confirm = Some(Confirm::QuitUnsaved);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if self.recovery_needs_init && !self.close_after_saves && !self.allow_close {
            self.attach_recovery();
        }
    }
}
#[cfg(test)]
mod tests;
