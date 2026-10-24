//! One bounded asynchronous raw capture followed by an acknowledged OS write.
use super::*;
use paintcraft_core::types::clipboard::Image;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_editor::clipboard::{Capture, capture};
use paintcraft_platform::clipboard::{ClipboardService, Job};
use paintcraft_ui::t;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

trait WriteJob {
    fn poll(&self) -> Option<Result<(), String>>;
}
impl WriteJob for Job {
    fn poll(&self) -> Option<Result<(), String>> {
        self.try_result()
    }
}
trait Writer {
    fn submit(&self, image: Image, ctx: &egui::Context) -> Result<Box<dyn WriteJob>, String>;
}
impl Writer for ClipboardService {
    fn submit(&self, image: Image, ctx: &egui::Context) -> Result<Box<dyn WriteJob>, String> {
        let ctx = ctx.clone();
        Ok(Box::new(self.submit(image, move || ctx.request_repaint())?))
    }
}

enum Stage {
    Capture(mpsc::Receiver<Result<Option<Capture>, String>>),
    Write {
        capture: Capture,
        job: Box<dyn WriteJob>,
    },
}
struct Pending {
    owner: Uuid,
    cancelled: Arc<AtomicBool>,
    stage: Stage,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub(super) struct State {
    writer: Option<Box<dyn Writer>>,
    pending: Option<Pending>,
    // Only acknowledged writes may replace this exact internal capture.
    last_capture: Option<Capture>,
}

fn budget() -> SharedBudget {
    static BUDGET: OnceLock<SharedBudget> = OnceLock::new();
    BUDGET.get_or_init(|| SharedBudget::new(128 << 20)).clone()
}

impl State {
    fn start(
        &mut self,
        owner: Uuid,
        document: Arc<paintcraft_document::Document>,
        selection: Selection,
        target: Uuid,
        ctx: &egui::Context,
    ) -> Result<bool, String> {
        if self.pending.is_some() {
            return Ok(false);
        }
        let (send, receive) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let wake = ctx.clone();
        std::thread::Builder::new()
            .name("paintcraft-copy-capture".into())
            .spawn(move || {
                let check = || {
                    if worker_cancelled.load(Ordering::Relaxed) {
                        Err(paintcraft_core::types::Error::new(
                            paintcraft_core::types::ErrorCode::OperationCancelled,
                            "Clipboard capture cancelled",
                        ))
                    } else {
                        Ok(())
                    }
                };
                let result = capture(&document, selection, target, &budget(), &check)
                    .map_err(|e| e.to_string());
                let _ = send.send(result);
                wake.request_repaint();
            })
            .map_err(|e| e.to_string())?;
        self.pending = Some(Pending {
            owner,
            cancelled,
            stage: Stage::Capture(receive),
        });
        Ok(true)
    }

    fn poll(&mut self, ctx: &egui::Context) -> Option<(Uuid, Result<bool, String>)> {
        let pending = self.pending.as_mut()?;
        let result = match &pending.stage {
            Stage::Capture(receiver) => match receiver.try_recv() {
                Ok(Ok(Some(capture))) => {
                    let submitted = if let Some(writer) = &self.writer {
                        writer.submit(capture.image.clone(), ctx)
                    } else {
                        ClipboardService::new().and_then(|writer| {
                            let result = Writer::submit(&writer, capture.image.clone(), ctx);
                            self.writer = Some(Box::new(writer));
                            result
                        })
                    };
                    match submitted {
                        Ok(job) => {
                            pending.stage = Stage::Write { capture, job };
                            return None;
                        }
                        Err(error) => Err(error),
                    }
                }
                Ok(Ok(None)) => Ok(false),
                Ok(Err(error)) => Err(error),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Err("Clipboard capture worker stopped".into())
                }
                Err(mpsc::TryRecvError::Empty) => return None,
            },
            Stage::Write { capture, job } => match job.poll()? {
                Ok(()) => {
                    self.last_capture = Some(capture.clone());
                    Ok(true)
                }
                Err(error) => Err(error),
            },
        };
        let owner = pending.owner;
        self.pending = None;
        Some((owner, result))
    }
}

impl App {
    pub(super) fn copy_requested(&mut self, ctx: &egui::Context) {
        if self.document_count() == 0 || self.clipboard.pending.is_some() {
            return;
        }
        if ctx.egui_wants_keyboard_input()
            || self.ime_composing
            || self.editor.is_editing_text()
            || self.has_document_drafts()
            || self.editor.has_floating_selection()
            || self.editor.is_constructing_shape()
            || self.editor.has_pending_edits()
        {
            self.editor
                .set_status(t!(ctx, "Finish the current edit before copying."));
            return;
        }
        let Some(target) = self.editor.target() else {
            self.editor
                .set_status(t!(ctx, "There are no selected pixels to copy."));
            return;
        };
        match self.clipboard.start(
            self.active_tab_id(),
            self.editor.document_snapshot(),
            self.editor.selection(),
            target,
            ctx,
        ) {
            Ok(true) => self.editor.set_status(t!(ctx, "Copying to clipboard…")),
            Ok(false) => {}
            Err(error) => self
                .editor
                .set_status(format!("{}: {error}", t!(ctx, "Clipboard copy failed"))),
        }
    }

    pub(super) fn poll_clipboard(&mut self, ctx: &egui::Context) {
        if let Some((owner, result)) = self.clipboard.poll(ctx) {
            let message = match result {
                Ok(true) => t!(ctx, "Copied to clipboard."),
                Ok(false) => t!(ctx, "There are no selected pixels to copy."),
                Err(error) => format!("{}: {error}", t!(ctx, "Clipboard copy failed")),
            };
            if owner == self.active_tab_id() {
                self.editor.set_status(message);
            } else if let Some(state) = self
                .tabs
                .iter_mut()
                .find(|tab| tab.id == owner)
                .and_then(|tab| tab.state.as_mut())
            {
                state.editor.set_status(message);
            }
        }
        if self.clipboard.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
#[path = "clipboard/tests.rs"]
mod tests;
