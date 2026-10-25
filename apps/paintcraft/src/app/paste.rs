//! Explicit clipboard read, bounded off-thread decode and source-owned placement.
use super::*;
use paintcraft_core::types::{
    clipboard::{Image, MAX_BYTES, MAX_PIXELS},
    resources::SharedBudget,
};
use paintcraft_editor::clipboard::PasteDraft;
use paintcraft_formats::{
    image,
    registry::{self, Frontend, Platform},
};
use paintcraft_platform::clipboard::{EncodedImage, Job, ReadService};
use paintcraft_ui::t;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
#[path = "paste/decode.rs"]
mod decode;
#[path = "paste/dialog.rs"]
mod dialog;
#[path = "paste/placement.rs"]
mod placement;
trait ReadJob {
    fn poll(&self) -> Option<Result<EncodedImage, String>>;
}
impl ReadJob for Job<EncodedImage> {
    fn poll(&self) -> Option<Result<EncodedImage, String>> {
        self.try_result()
    }
}
trait Reader {
    fn supported(&self) -> bool;
    fn submit(&self, budget: SharedBudget, ctx: &egui::Context)
    -> Result<Box<dyn ReadJob>, String>;
}
impl Reader for ReadService {
    fn supported(&self) -> bool {
        ReadService::supported()
    }
    fn submit(
        &self,
        budget: SharedBudget,
        ctx: &egui::Context,
    ) -> Result<Box<dyn ReadJob>, String> {
        let ctx = ctx.clone();
        Ok(Box::new(
            self.submit(budget, move || ctx.request_repaint())?,
        ))
    }
}
#[derive(Clone)]
struct Owner {
    tab: Uuid,
    revision: (Uuid, u64),
    document: Arc<paintcraft_document::Document>,
    frontend: Frontend,
    target: Option<Uuid>,
}
impl Owner {
    fn capture(app: &App) -> Self {
        Self {
            tab: app.active_tab_id(),
            revision: app.editor.render_key(),
            document: app.editor.document_snapshot(),
            frontend: app.format_frontend(),
            target: None,
        }
    }
    fn current(&self, app: &App) -> bool {
        app.document_count() > 0
            && self.tab == app.active_tab_id()
            && self.revision == app.editor.render_key()
            && Arc::ptr_eq(&self.document, &app.editor.document_snapshot())
            && self.frontend == app.format_frontend()
            && self
                .target
                .is_none_or(|target| app.editor.target() == Some(target))
            && !app.has_document_drafts()
            && !app.editor.has_floating_selection()
            && !app.editor.is_constructing_shape()
            && !app.editor.has_pending_edits()
            && !app.ime_composing
            && !app.closing_workspace
            && app.switching.is_none()
            && app.save_all.is_none()
    }
}
struct Decoded {
    image: Image,
    losses: Vec<String>,
}
enum Stage {
    Read(Box<dyn ReadJob>),
    Decode(mpsc::Receiver<Result<Decoded, String>>),
}
struct Pending {
    owner: Owner,
    cancelled: Arc<AtomicBool>,
    stage: Stage,
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
struct Ready {
    owner: Owner,
    draft: PasteDraft,
    losses: Vec<String>,
    accept_losses: bool,
    origin: (i64, i64),
    grow: bool,
    name: String,
}
pub(super) struct State {
    reader: Option<Box<dyn Reader>>,
    pending: Option<Pending>,
    ready: Option<Ready>,
    raw_budget: SharedBudget,
}
impl Default for State {
    fn default() -> Self {
        Self {
            reader: None,
            pending: None,
            ready: None,
            raw_budget: clipboard::budget(),
        }
    }
}
impl State {
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some() || self.ready.is_some()
    }
    pub(super) fn supported(&self) -> bool {
        self.reader
            .as_ref()
            .map_or_else(ReadService::supported, |reader| reader.supported())
    }
}
fn encoded_budget() -> SharedBudget {
    static B: OnceLock<SharedBudget> = OnceLock::new();
    B.get_or_init(|| SharedBudget::new(64 << 20)).clone()
}
impl App {
    pub(super) fn paste_layer_requested(&mut self, ctx: &egui::Context) {
        self.begin_clipboard_paste(ctx, false);
    }
    pub(super) fn paste_requested(&mut self, ctx: &egui::Context) {
        self.begin_clipboard_paste(ctx, true);
    }
    fn begin_clipboard_paste(&mut self, ctx: &egui::Context, onto: bool) {
        if self.paste.busy() || self.clipboard.busy() || self.document_count() == 0 {
            return;
        }
        if !Owner::capture(self).current(self)
            || ctx.egui_wants_keyboard_input()
            || self.confirm.is_some()
        {
            self.editor
                .set_status(t!(ctx, "Finish the current edit before pasting."));
            return;
        }
        let mut owner = Owner::capture(self);
        if onto {
            owner.target = self.editor.target().filter(|id| {
                self.editor
                    .document()
                    .layers
                    .get(id)
                    .is_some_and(|layer| layer.visible)
            });
            if owner.target.is_none() {
                return;
            }
        }
        let result = (|| {
            if self.paste.reader.is_none() {
                self.paste.reader = Some(Box::new(ReadService::new()?));
            }
            let job = self
                .paste
                .reader
                .as_ref()
                .expect("reader")
                .submit(encoded_budget(), ctx)?;
            Ok::<_, String>(Pending {
                owner,
                cancelled: Arc::new(AtomicBool::new(false)),
                stage: Stage::Read(job),
            })
        })();
        match result {
            Ok(pending) => {
                self.paste.pending = Some(pending);
                self.editor.set_status(t!(ctx, "Reading clipboard…"));
            }
            Err(error) => self.paste_error(ctx, &error),
        }
    }
    pub(super) fn cancel_paste(&mut self, ctx: &egui::Context) {
        self.editor.cancel_image_paste();
        if self.paste.busy() {
            let owner = self
                .paste
                .pending
                .as_ref()
                .map(|pending| pending.owner.tab)
                .or_else(|| self.paste.ready.as_ref().map(|ready| ready.owner.tab))
                .expect("Paste owner");
            self.paste.pending = None;
            self.paste.ready = None;
            if self.confirm == Some(Confirm::PasteLayer) {
                self.confirm = None;
            }
            self.clipboard_status(owner, t!(ctx, "Paste cancelled."));
        }
    }
    fn paste_error(&mut self, ctx: &egui::Context, error: &str) {
        self.editor
            .set_status(format!("{}: {error}", t!(ctx, "Clipboard paste failed")));
    }
    pub(super) fn poll_paste(&mut self, ctx: &egui::Context) {
        if self.paste.ready.is_some() && self.confirm != Some(Confirm::PasteLayer) {
            self.paste.ready = None;
        }
        let Some(mut pending) = self.paste.pending.take() else {
            return;
        };
        if !pending.owner.current(self) || self.confirm.is_some() {
            self.clipboard_status(
                pending.owner.tab,
                t!(ctx, "The document changed; paste again."),
            );
            return;
        }
        match &pending.stage {
            Stage::Read(job) => match job.poll() {
                None => {}
                Some(Err(error)) => {
                    self.paste_error(ctx, &error);
                    return;
                }
                Some(Ok(encoded)) => {
                    let reservation = match self.paste.raw_budget.reserve_capacity(MAX_BYTES) {
                        Ok(value) => value,
                        Err(error) => {
                            self.paste_error(ctx, &error.to_string());
                            return;
                        }
                    };
                    let (send, receive) = mpsc::sync_channel(1);
                    let cancelled = pending.cancelled.clone();
                    let wake = ctx.clone();
                    let frontend = pending.owner.frontend;
                    let result = std::thread::Builder::new()
                        .name("paintcraft-paste-decode".into())
                        .spawn(move || {
                            let result = decode::source(encoded, frontend, reservation, cancelled)
                                .map_err(|error| error.to_string());
                            let _ = send.send(result);
                            wake.request_repaint();
                        });
                    if let Err(error) = result {
                        self.paste_error(ctx, &error.to_string());
                        return;
                    }
                    pending.stage = Stage::Decode(receive);
                }
            },
            Stage::Decode(receiver) => match receiver.try_recv() {
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.paste_error(ctx, "Clipboard decoder stopped");
                    return;
                }
                Ok(Err(error)) => {
                    self.paste_error(ctx, &error);
                    return;
                }
                Ok(Ok(decoded)) => {
                    match self.editor.paste_draft(decoded.image) {
                        Ok(draft) => {
                            self.paste.ready = Some(Ready {
                                owner: pending.owner.clone(),
                                draft,
                                losses: decoded.losses,
                                accept_losses: false,
                                origin: (0, 0),
                                grow: false,
                                name: t!(ctx, "Pasted image"),
                            });
                            self.confirm = Some(Confirm::PasteLayer);
                        }
                        Err(error) => self.paste_error(ctx, &error.to_string()),
                    }
                    return;
                }
            },
        }
        self.paste.pending = Some(pending);
        ctx.request_repaint_after(Duration::from_millis(30));
    }
}
#[cfg(test)]
#[path = "paste/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "paste/dib_tests.rs"]
mod dib_tests;
