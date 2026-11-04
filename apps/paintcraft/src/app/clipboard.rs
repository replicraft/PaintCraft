//! One bounded asynchronous raw capture followed by an acknowledged OS write.
use super::*;
use paintcraft_core::types::clipboard::Image;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_editor::clipboard::{Capture, CutSource, capture};
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
    cut: Option<CutSource>,
    stage: Stage,
    _spatial_parent: Option<paintcraft_core::types::resources::BudgetLease>,
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
    offset: (i64, i64),
    cut_float: Option<(Uuid, Option<u64>)>,
}

pub(super) fn budget() -> SharedBudget {
    static BUDGET: OnceLock<SharedBudget> = OnceLock::new();
    BUDGET.get_or_init(|| SharedBudget::new(128 << 20)).clone()
}

impl State {
    pub(super) fn matching_origin(&self, image: &Image) -> Option<(i64, i64)> {
        self.last_capture
            .as_ref()
            .filter(|capture| {
                capture.image.width() == image.width()
                    && capture.image.height() == image.height()
                    && capture.image.rgba() == image.rgba()
            })
            .map(|capture| capture.origin)
    }
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        owner: Uuid,
        document: Arc<paintcraft_document::Document>,
        selection: Selection,
        target: Option<Uuid>,
        cut: Option<CutSource>,
        ctx: &egui::Context,
        parent: &SharedBudget,
    ) -> Result<bool, String> {
        if self.pending.is_some() {
            return Ok(false);
        }
        let spatial = target.is_none() && spatial_workspace::needed(&document);
        let policy = if target.is_none() {
            spatial_workspace::JobPolicy::capture(&document, None, spatial)
        } else {
            // Raw target capture does not evaluate any editable filter.
            spatial_workspace::JobPolicy::None
        };
        let spatial_parent = if spatial {
            Some(
                parent
                    .reserve(spatial_workspace::BYTES)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        let executing_parent = spatial_parent.clone();
        let (send, receive) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let wake = ctx.clone();
        let offset = self.offset;
        std::thread::Builder::new()
            .name("paintcraft-copy-capture".into())
            .spawn(move || {
                let _parent = executing_parent;
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
                let result = policy
                    .run(|| match target {
                        Some(target) => capture(&document, selection, target, &budget(), &check),
                        None => paintcraft_render::clipboard::capture_visible(
                            &document,
                            &selection,
                            &budget(),
                            &check,
                        )
                        .map(|image| {
                            image.map(|(image, origin)| Capture {
                                image,
                                origin,
                                selection,
                            })
                        }),
                    })
                    .map(|capture| {
                        capture.map(|mut capture| {
                            capture.origin.0 += offset.0;
                            capture.origin.1 += offset.1;
                            capture
                        })
                    })
                    .map_err(|e| e.to_string());
                // The pending write retains its own reservation until acknowledgment.
                // Retire the executing clone before publishing completion.
                drop(_parent);
                let _ = send.send(result);
                wake.request_repaint();
            })
            .map_err(|e| e.to_string())?;
        self.pending = Some(Pending {
            owner,
            cancelled,
            cut,
            stage: Stage::Capture(receive),
            _spatial_parent: spatial_parent,
        });
        Ok(true)
    }

    fn poll(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<(Uuid, Result<bool, String>, Option<CutSource>)> {
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
        let cut = pending.cut.take();
        self.pending = None;
        Some((owner, result, cut))
    }
}

impl App {
    pub(super) fn copy_requested(&mut self, ctx: &egui::Context) {
        self.copy_source_requested(ctx, false);
    }

    pub(super) fn copy_source_requested(&mut self, ctx: &egui::Context, visible: bool) {
        self.clipboard_requested(ctx, visible, false);
    }

    pub(super) fn cut_requested(&mut self, ctx: &egui::Context) {
        self.clipboard_requested(ctx, false, true);
    }

    fn clipboard_requested(&mut self, ctx: &egui::Context, visible: bool, cut: bool) {
        if self.document_count() == 0 || self.clipboard.pending.is_some() || self.paste.busy() {
            return;
        }
        if ctx.text_edit_focused()
            || self.ime_composing
            || self.editor.is_editing_text()
            || self.editor.pointer_is_down()
            || self.has_layer_property_drafts()
            || (visible && self.editor.has_floating_selection())
            || self.editor.is_constructing_shape()
            || self.editor.has_pending_edits()
        {
            self.editor
                .set_status(t!(ctx, "Finish the current edit before copying."));
            return;
        }
        let (document, selection, target, offset) = self.editor.clipboard_source();
        if !visible && target.is_none() {
            self.editor
                .set_status(t!(ctx, "There are no selected pixels to copy."));
            return;
        }
        let cut = if cut {
            let clear = if self.experience() == Experience::Studio {
                paintcraft_core::types::Rgba::TRANSPARENT
            } else {
                self.editor.canvas_color()
            };
            match self.editor.cut_source(clear) {
                Ok(source) => Some(source),
                Err(error) => {
                    self.editor
                        .set_status(format!("{}: {error}", t!(ctx, "Clipboard copy failed")));
                    return;
                }
            }
        } else {
            None
        };
        self.clipboard.offset = offset;
        self.clipboard.cut_float = (cut.is_some() && self.editor.has_floating_selection())
            .then(|| self.editor.preview_owner());
        match self.clipboard.start(
            self.active_tab_id(),
            document,
            selection,
            if visible { None } else { target },
            cut,
            ctx,
            &self.worker_budget,
        ) {
            Ok(true) => self.editor.set_status(t!(ctx, "Copying to clipboard…")),
            Ok(false) => {}
            Err(error) => self
                .editor
                .set_status(format!("{}: {error}", t!(ctx, "Clipboard copy failed"))),
        }
    }

    pub(super) fn poll_clipboard(&mut self, ctx: &egui::Context) {
        if let Some((owner, result, cut)) = self.clipboard.poll(ctx) {
            if result == Ok(true)
                && let Some(cut) = cut
            {
                let floating = self.clipboard.cut_float.take();
                let floating_current = match floating {
                    Some(owner) => {
                        self.editor.has_floating_selection() && owner == self.editor.preview_owner()
                    }
                    None => !self.editor.has_floating_selection(),
                };
                let unchanged = self.document_count() > 0
                    && owner == self.active_tab_id()
                    && !self.ime_composing
                    && !self.editor.pointer_is_down()
                    && !self.has_layer_property_drafts()
                    && !self.editor.is_editing_text()
                    && floating_current
                    && !self.editor.is_constructing_shape()
                    && !self.editor.has_pending_edits();
                if unchanged {
                    if floating.is_some() {
                        self.editor.cancel_gesture();
                    }
                }
                if unchanged && self.editor.complete_cut(&cut).is_ok() {
                    return;
                }
                self.clipboard_status(
                    owner,
                    t!(
                        ctx,
                        "Copied, but Cut was cancelled because the source changed."
                    ),
                );
                return;
            }
            let message = match result {
                Ok(true) => t!(ctx, "Copied to clipboard."),
                Ok(false) => t!(ctx, "There are no selected pixels to copy."),
                Err(error) => format!("{}: {error}", t!(ctx, "Clipboard copy failed")),
            };
            self.clipboard_status(owner, message);
        }
        if self.clipboard.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    pub(super) fn clipboard_status(&mut self, owner: Uuid, message: String) {
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
    pub(super) fn cancel_clipboard_cut(&mut self, ctx: &egui::Context) {
        if self
            .clipboard
            .pending
            .as_ref()
            .is_some_and(|pending| pending.cut.is_some())
        {
            let pending = self.clipboard.pending.take().expect("pending Cut");
            self.clipboard_status(
                pending.owner,
                t!(ctx, "Cut cancelled; the canvas was not changed."),
            );
        }
    }
}

#[cfg(test)]
#[path = "clipboard/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "clipboard/noise_tests.rs"]
mod noise_tests;
