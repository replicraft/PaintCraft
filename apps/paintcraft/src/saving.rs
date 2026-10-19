//! Native file coordinator. Encoding receives immutable content and a bounded
//! staging sink. A single FIFO also serializes spelling/symlink aliases of paths.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result, json::Value};
use paintcraft_runtime::io::{Limits, OrderedIo, Ticket};

#[derive(Debug)]
struct Pending {
    ticket: Ticket,
    session: Uuid,
    generation: u64,
    source: Uuid,
    path: PathBuf,
    operation: &'static str,
}
#[derive(Debug)]
pub(super) struct NativeSaving {
    io: OrderedIo<save::Outcome>,
    session: Uuid,
    generation: u64,
    pending: Vec<Pending>,
    failed: bool,
    last_receipt: Option<Value>,
}
impl NativeSaving {
    pub(super) fn new() -> Result<Self> {
        Ok(Self {
            io: OrderedIo::new(Limits {
                pending_jobs: 4,
                retained_bytes: 1 << 30,
            })?,
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
        // Provisional conservative payload charge, independently charged per snapshot.
        // 64 MiB reserves encoder chunk, metadata and directory scratch. This does not
        // claim allocator/RSS accounting or substitute for the future residency service.
        let charge =
            document
                .layers
                .values()
                .try_fold(64u64 << 20, |bytes, layer| {
                    bytes
                        .checked_add((layer.raster.resident_tiles() as u64).checked_mul(
                            paintcraft_raster::RasterLayer::TILE_BYTES as u64 + 8192,
                        )?)?
                        .checked_add(layer.name.len() as u64 + 4096)
                })
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "save retention charge overflows")
                })?;
        let destination = path.clone();
        let ticket = self.io.submit(
            charge,
            move || {
                Ok(save::save_with(
                    &destination,
                    true,
                    save::STAGED_FILE_BYTES_MAX,
                    |sink| paintcraft_formats::pcnd::write_to(&document, sink),
                ))
            },
            move || ctx.request_repaint(),
        )?;
        self.generation = generation;
        if self.pending.is_empty() {
            self.failed = false;
        }
        self.pending.push(Pending {
            ticket,
            session: self.session,
            generation,
            source,
            path,
            operation,
        });
        Ok(())
    }
}
impl App {
    pub(super) fn close_after_requested_saves(&mut self, ctx: &egui::Context) {
        if self.saving.as_ref().is_some_and(NativeSaving::pending) {
            self.close_after_saves = true;
            self.editor
                .set_status("Waiting for requested saves before closing…");
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub(super) fn poll_native_saves(&mut self, ctx: &egui::Context) {
        let Some(saving) = &mut self.saving else {
            return;
        };
        while let Some((ticket, result)) = saving.io.poll() {
            let index = saving
                .pending
                .iter()
                .position(|p| p.ticket == ticket)
                .expect("admitted save");
            let pending = saving.pending.remove(index);
            let current =
                pending.session == saving.session && pending.generation == saving.generation;
            match result {
                Ok(outcome) => {
                    let success = outcome.is_success();
                    saving.failed |= !success;
                    saving.last_receipt = Some(outcome.receipt_stamped(
                        &save::Context {
                            operation: pending.operation,
                            source_state: pending.source,
                            source_sha256: None,
                            losses_acknowledged: vec![],
                            destination: &pending.path,
                        },
                        ticket.sequence(),
                        pending.generation,
                    ));
                    if current && success {
                        self.editor.set_path(pending.path.clone());
                        self.path_text = pending.path.to_string_lossy().into_owned();
                        self.bound = Some(pending.path.clone());
                        self.editor.acknowledge_saved_state(pending.source);
                        self.editor.set_status(format!(
                            "Saved {}{}",
                            pending.path.display(),
                            if self.editor.is_dirty() {
                                "; newer edits remain unsaved"
                            } else {
                                ""
                            }
                        ));
                    }
                    if !success {
                        self.editor.set_status(format!(
                            "{} was not saved safely: {}",
                            pending.path.display(),
                            outcome.reasons.join("; ")
                        ));
                    }
                }
                Err(error) => {
                    saving.failed = true;
                    self.editor.set_status(format!(
                        "Save outcome unconfirmed for {}: {error}",
                        pending.path.display()
                    ));
                }
            }
        }
        if self.close_after_saves && !saving.pending() {
            self.close_after_saves = false;
            if saving.failed {
                self.allow_close = false;
                self.editor.set_status("A requested save failed or remains unconfirmed. Review the document before closing.");
            } else if self.editor.is_dirty() && !self.allow_close {
                self.confirm = Some(Confirm::QuitUnsaved);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
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
        app.saving = Some(NativeSaving::new().unwrap());
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
                    Ok(save::save(&dir.join("barrier"), b"barrier", false))
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
        app.install(Editor::new_document(64, 64).unwrap());
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
