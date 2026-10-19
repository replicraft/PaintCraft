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
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Instant};
    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!("paintcraft-ordered-save-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        path
    }
    fn app() -> App {
        let mut app = App::new(None);
        app.saving = Some(NativeSaving::new(egui::Context::default()).unwrap());
        app
    }
    fn recovery_app(directory: PathBuf) -> App {
        let mut app = app().with_recovery(directory);
        drain(&mut app);
        app
    }
    fn drain(app: &mut App) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.saving.as_ref().unwrap().pending() {
            app.poll_native_saves(&egui::Context::default());
            assert!(Instant::now() < deadline, "save queue did not finish");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn submit(app: &mut App, path: PathBuf) {
        app.saving
            .as_mut()
            .unwrap()
            .submit(
                &app.editor,
                path,
                app.bound.as_deref(),
                egui::Context::default(),
            )
            .unwrap();
    }
    #[test]
    fn snapshot_a_saves_while_b_stays_dirty_and_undo_a_is_clean() {
        let dir = scratch();
        let path = dir.join("a.pcnd");
        let mut app = app();
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let saved = app.editor.document_snapshot();
        let (release, hold) = mpsc::channel();
        app.saving
            .as_mut()
            .unwrap()
            .io
            .submit(
                1,
                move || {
                    hold.recv().unwrap();
                    Ok(Completion::File(save::save(
                        &dir.join("barrier"),
                        b"barrier",
                        false,
                    )))
                },
                || {},
            )
            .unwrap();
        // This barrier is a coordinator test job, so consume it separately before
        // normal App completion handling (which owns only admitted user-save tickets).
        submit(&mut app, path.clone());
        app.editor.request(2, Action::AddLayer);
        app.editor.run_pending();
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if app.saving.as_mut().unwrap().io.poll().is_some() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        drain(&mut app);
        assert!(app.editor.is_dirty());
        let stored = Editor::open(&path).unwrap();
        assert!(stored.document().same_semantics(&saved));
        app.editor.request(3, Action::Undo);
        app.editor.run_pending();
        assert!(!app.editor.is_dirty());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn later_save_to_same_destination_wins_and_receipts_keep_real_sequence() {
        let dir = scratch();
        let path = dir.join("a.pcnd");
        let mut app = app();
        submit(&mut app, path.clone());
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        submit(&mut app, path.clone());
        drain(&mut app);
        assert!(
            Editor::open(&path)
                .unwrap()
                .document()
                .same_semantics(app.editor.document())
        );
        assert!(!app.editor.is_dirty());
        let receipt = app.saving.as_ref().unwrap().last_receipt.as_ref().unwrap();
        assert_eq!(
            receipt.get("save_sequence").and_then(Value::as_str),
            Some("2")
        );
        assert_eq!(
            receipt.get("binding_generation").and_then(Value::as_str),
            Some("2")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn stale_save_as_completion_cannot_retarget_later_failed_choice() {
        let dir = scratch();
        let original = dir.join("original.pcnd");
        let mut app = app();
        app.bound = Some(original.clone());
        app.editor.set_path(original.clone());
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        submit(&mut app, dir.join("older-choice.pcnd"));
        submit(&mut app, dir.join("missing/latest-choice.pcnd"));
        drain(&mut app);
        assert_eq!(app.bound, Some(original.clone()));
        assert_eq!(app.editor.path(), Some(original.as_path()));
        assert!(app.editor.is_dirty());
        assert!(
            dir.join("older-choice.pcnd").exists(),
            "external successful write is retained despite stale association"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn completion_from_replaced_editor_cannot_clean_or_bind_the_new_editor() {
        let dir = scratch();
        let mut app = app();
        submit(&mut app, dir.join("old-session.pcnd"));
        app.install(Editor::new_document(64, 64).unwrap()).unwrap();
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        drain(&mut app);
        assert!(app.bound.is_none());
        assert!(app.editor.path().is_none());
        assert!(app.editor.is_dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn closing_waits_for_accepted_saves_and_failure_keeps_the_window_open() {
        let dir = scratch();
        let mut app = app();
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        submit(&mut app, dir.join("missing/fails.pcnd"));
        app.allow_close = true;
        app.close_after_requested_saves(&egui::Context::default());
        assert!(
            app.close_after_saves,
            "accepted save must drain even after quit confirmation"
        );
        drain(&mut app);
        assert!(!app.close_after_saves);
        assert!(
            !app.allow_close,
            "failed save needs a new explicit decision"
        );
        assert!(app.editor.is_dirty());
        assert!(
            app.editor
                .status()
                .contains("failed or remains unconfirmed")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn pause_queue(app: &mut App) -> mpsc::Sender<()> {
        let (release, hold) = mpsc::channel();
        let saving = app.saving.as_mut().unwrap();
        let ticket = saving
            .io
            .submit(
                0,
                move || {
                    hold.recv().unwrap();
                    Ok(Completion::Cleanup)
                },
                || {},
            )
            .unwrap();
        saving.pending.push(Pending {
            ticket,
            kind: PendingKind::Cleanup,
        });
        release
    }
    fn checkpoint(app: &mut App) -> bool {
        app.saving
            .as_mut()
            .unwrap()
            .checkpoint(&app.editor)
            .unwrap()
    }
    #[test]
    fn background_checkpoint_captures_a_without_blocking_edits_to_b() {
        let dir = scratch();
        let mut app = recovery_app(dir.clone());
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let a = app.editor.document_snapshot();
        let release = pause_queue(&mut app);
        assert!(checkpoint(&mut app));
        assert!(
            !app.editor.has_checkpoint(),
            "submission is not a durable checkpoint"
        );
        app.editor.request(2, Action::AddLayer);
        app.editor.run_pending();
        assert!(
            !checkpoint(&mut app),
            "only one current-owner checkpoint is admitted at once"
        );
        release.send(()).unwrap();
        drain(&mut app);
        assert_eq!(app.editor.checkpoint_info().unwrap().state_id, a.state_id);
        assert_ne!(app.editor.document().state_id, a.state_id);
        let b = app.editor.document_snapshot();
        assert!(checkpoint(&mut app));
        drain(&mut app);
        assert_eq!(app.editor.checkpoint_info().unwrap().state_id, b.state_id);
        drop(app);
        let found = Recovery::scan(&dir);
        assert_eq!(found.len(), 1);
        assert!(
            Recovery::read_checkpoint(&found[0].checkpoint)
                .unwrap()
                .same_semantics(&b)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn checkpoint_clear_and_new_checkpoint_are_fenced_and_ordered() {
        let dir = scratch();
        let mut app = recovery_app(dir.clone());
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let release = pause_queue(&mut app);
        assert!(checkpoint(&mut app));
        app.saving
            .as_mut()
            .unwrap()
            .clear_recovery(&mut app.editor)
            .unwrap();
        app.editor.request(2, Action::AddLayer);
        app.editor.run_pending();
        let expected = app.editor.document_snapshot();
        assert!(checkpoint(&mut app));
        release.send(()).unwrap();
        drain(&mut app);
        assert_eq!(
            app.editor.checkpoint_info().unwrap().sequence,
            1,
            "old queued checkpoint was superseded before publication"
        );
        assert_eq!(
            app.editor.checkpoint_info().unwrap().state_id,
            expected.state_id
        );
        drop(app);
        let found = Recovery::scan(&dir);
        assert_eq!(found.len(), 1);
        assert!(
            Recovery::read_checkpoint(&found[0].checkpoint)
                .unwrap()
                .same_semantics(&expected)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn replacing_editor_does_not_wait_for_old_checkpoint_or_resurrect_it() {
        let dir = scratch();
        let mut app = recovery_app(dir.clone());
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let release = pause_queue(&mut app);
        assert!(checkpoint(&mut app));
        let installed = app.install(Editor::new_document(64, 64).unwrap());
        release.send(()).unwrap();
        installed.unwrap();
        assert_eq!(app.editor.document().canvas, (64, 64));
        drain(&mut app);
        assert!(app.editor.checkpoint_info().is_none());
        drop(app);
        assert!(Recovery::scan(&dir).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn full_queue_refuses_document_replacement_before_invalidating_recovery() {
        let dir = scratch();
        let mut app = recovery_app(dir.clone());
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let token = app.editor.recovery_handle().unwrap().token();
        let key = app.editor.render_key();
        let release = pause_queue(&mut app);
        for _ in 0..3 {
            let saving = app.saving.as_mut().unwrap();
            let ticket = saving
                .io
                .submit(1, || Ok(Completion::Cleanup), || {})
                .unwrap();
            saving.pending.push(Pending {
                ticket,
                kind: PendingKind::Cleanup,
            });
        }
        let refused = app.install(Editor::new_document(64, 64).unwrap());
        release.send(()).unwrap();
        assert_eq!(refused.unwrap_err().code, ErrorCode::ResourceLimit);
        assert_eq!(app.editor.render_key(), key);
        assert!(app.editor.recovery_handle().unwrap().is_current(token));
        drain(&mut app);
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn clean_save_drains_recovery_cleanup_without_adopting_a_stale_checkpoint() {
        let dir = scratch();
        let recovery_dir = dir.join("recovery");
        let path = dir.join("saved.pcnd");
        let mut app = recovery_app(recovery_dir.clone());
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let expected = app.editor.document_snapshot();
        let release = pause_queue(&mut app);
        submit(&mut app, path.clone());
        assert!(checkpoint(&mut app));
        release.send(()).unwrap();
        drain(&mut app);
        assert!(!app.editor.is_dirty());
        assert!(app.editor.checkpoint_info().is_none());
        assert!(
            Editor::open(&path)
                .unwrap()
                .document()
                .same_semantics(&expected)
        );
        drop(app);
        assert!(Recovery::scan(&recovery_dir).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn undo_to_saved_state_closes_after_clearing_a_newer_recovery_snapshot() {
        let dir = scratch();
        let mut app = recovery_app(dir.clone());
        // The initial state is clean. B is checkpointed, then undo returns to A.
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        assert!(checkpoint(&mut app));
        drain(&mut app);
        assert!(app.editor.checkpoint_info().is_some());
        app.editor.request(2, Action::Undo);
        app.editor.run_pending();
        assert!(!app.editor.is_dirty());
        app.close_after_requested_saves(&egui::Context::default());
        assert!(app.close_after_saves);
        drain(&mut app);
        assert!(app.editor.checkpoint_info().is_none());
        drop(app);
        assert!(
            Recovery::scan(&dir).is_empty(),
            "undone edits must not reappear after clean exit"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn startup_discovery_and_initialization_leave_the_editor_responsive() {
        let dir = scratch();
        let mut app = app();
        let release = pause_queue(&mut app);
        app = app.with_recovery(dir.clone());
        assert!(app.editor.recovery_handle().is_none());
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "configuration must not create files on the UI path"
        );
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        let key = app.editor.render_key();
        release.send(()).unwrap();
        drain(&mut app);
        assert!(app.editor.recovery_handle().is_some());
        assert_eq!(
            app.editor.render_key(),
            key,
            "initialization must not replace edits made while scanning"
        );
        assert!(!app.recovery_needs_scan && !app.recovery_needs_init);
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn stale_startup_owner_is_retired_and_the_new_editor_gets_its_own_owner() {
        let dir = scratch();
        let mut app = app();
        let release = pause_queue(&mut app);
        app = app.with_recovery(dir.clone());
        app.install(Editor::new_document(64, 64).unwrap()).unwrap();
        assert!(
            app.recovery_needs_init,
            "exclusive discovery retains its admission until consumed"
        );
        release.send(()).unwrap();
        drain(&mut app);
        assert!(app.editor.recovery_handle().is_some());
        assert_eq!(app.editor.document().canvas, (64, 64));
        assert!(!app.recovery_needs_init);
        drop(app);
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "stale empty owners must be cleaned up"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn make_orphan(directory: &Path) -> Uuid {
        let recovery = Recovery::start(directory).unwrap();
        recovery
            .checkpoint_document(Editor::new_document(4, 4).unwrap().document(), None)
            .unwrap();
        recovery.session()
    }
    #[test]
    fn discovery_results_follow_the_current_directory_configuration() {
        let a = scratch();
        let b = scratch();
        let a_id = make_orphan(&a);
        let b_id = make_orphan(&b);
        let mut app = app();
        let release = pause_queue(&mut app);
        app = app.with_recovery(a.clone()).with_recovery(b.clone());
        release.send(()).unwrap();
        drain(&mut app);
        assert_eq!(app.recoverable.len(), 1);
        assert_eq!(app.recoverable[0].session, b_id);
        assert_ne!(app.recoverable[0].session, a_id);
        assert_eq!(
            Recovery::scan(&a).len(),
            1,
            "other directory's orphan is preserved"
        );
        drop(app);
        std::fs::remove_dir_all(a).unwrap();
        std::fs::remove_dir_all(b).unwrap();
    }
    #[test]
    fn recovery_initialization_failure_does_not_stop_the_editor_or_invent_protection() {
        let dir = scratch();
        let blocked = dir.join("not-a-directory");
        std::fs::write(&blocked, b"blocked").unwrap();
        let mut app = app().with_recovery(blocked);
        drain(&mut app);
        assert!(app.editor.recovery_handle().is_none());
        assert!(!app.recovery_needs_init);
        assert!(app.editor.status().contains("Crash recovery unavailable"));
        app.editor.request(1, Action::AddLayer);
        assert_eq!(app.editor.run_pending(), 1);
        drop(app);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn startup_configuration_does_not_silently_migrate_an_active_store() {
        let a = scratch();
        let b = scratch();
        let mut app = recovery_app(a.clone());
        let token = app.editor.recovery_handle().unwrap().token();
        app.editor.request(1, Action::AddLayer);
        app.editor.run_pending();
        assert!(checkpoint(&mut app));
        drain(&mut app);
        app = app.with_recovery(b.clone());
        assert_eq!(app.recovery_dir.as_deref(), Some(a.as_path()));
        assert!(app.editor.recovery_handle().unwrap().is_current(token));
        assert!(app.editor.has_checkpoint());
        assert!(
            app.editor
                .status()
                .contains("Existing recovery data has been retained")
        );
        assert_eq!(std::fs::read_dir(&b).unwrap().count(), 0);
        drop(app);
        assert_eq!(Recovery::scan(&a).len(), 1);
        std::fs::remove_dir_all(a).unwrap();
        std::fs::remove_dir_all(b).unwrap();
    }

    #[test]
    fn repeated_layout_passes_submit_one_save() {
        let dir = scratch();
        let mut app = app();
        let ctx = egui::Context::default();
        app.do_save(&ctx, dir.join("a.pcnd"));
        app.do_save(&ctx, dir.join("a.pcnd"));
        assert_eq!(app.saving.as_ref().unwrap().pending.len(), 1);
        drain(&mut app);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
