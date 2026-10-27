//! Opt-in advanced browser MSP2 uses the one host-installed isolated service.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::msp::web_export as wire;
use paintcraft_ui::msp_export::{self as controls, Version};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};
#[cfg(target_arch = "wasm32")]
mod bridge;
#[cfg(any(target_arch = "wasm32", test))]
mod response;
#[cfg(test)]
mod tests;
mod view;
#[cfg(any(target_arch = "wasm32", test))]
fn failed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "The document changed; choose the image again.",
    )
}
#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
#[derive(Debug)]
struct Prepared {
    header: wire::Header,
    response: Vec<u8>,
    encoded: Range<usize>,
    preview: Option<egui::ColorImage>,
    losses: Vec<String>,
    #[cfg(target_arch = "wasm32")]
    lease: Option<bridge::Lease>,
}
impl Prepared {
    #[cfg(test)]
    fn bytes(&self) -> &[u8] {
        &self.response[self.encoded.clone()]
    }
}
type Outcome<T> = Rc<RefCell<Option<Result<T>>>>;
#[derive(Debug)]
struct Pending {
    version: Version,
    result: Outcome<Prepared>,
    cancelled: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    cancellation: Rc<RefCell<Option<bridge::Cancellation>>>,
}
struct Review {
    owner: crate::importing::Owner,
    name: String,
    filename: String,
    version: Version,
    pending: Option<Pending>,
    prepared: Option<Prepared>,
    texture: Option<egui::TextureHandle>,
    shown: Option<u64>,
    approved: bool,
    error: Option<Error>,
}
impl std::fmt::Debug for Review {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MspReview")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}
impl Review {
    fn current(&self, app: &WebApp) -> bool {
        self.owner.current(app)
            && self.name == app.name
            && self.version == app.msp_version
            && self.version == Version::RowRleV2
            && app.frontend == Frontend::Studio
            && app.export_confirm == Some(ImageExport::Msp)
            && !app.files.busy
            && !app.jxl.busy()
            && !app.settings
            && app.form.is_none()
            && app.confirm_replace.is_none()
            && !app.welcome.open()
            && !app.paste.busy()
            && !app.editor.tools.is_constructing_shape()
            && app.editor.tools.floating_selection().is_none()
    }
    fn take_ready(&mut self, frame: u64) -> Result<Prepared> {
        let prepared = self.prepared.as_ref().ok_or_else(stale)?;
        if self.version != Version::RowRleV2
            || prepared.header.owner != self.owner.wire()
            || (!prepared.losses.is_empty() && !self.approved)
            || self.shown.is_none_or(|f| f >= frame)
        {
            return Err(stale());
        }
        self.prepared.take().ok_or_else(stale)
    }
    fn retire_preview(&mut self) {
        self.prepared = None;
        self.texture = None;
        self.shown = None;
        self.approved = false;
        self.error = None;
    }
}
#[derive(Debug, Default)]
pub(crate) struct State {
    review: Option<Review>,
    download: Option<(crate::importing::Owner, Outcome<()>)>,
    #[cfg(target_arch = "wasm32")]
    service: Option<wasm_bindgen::JsValue>,
}
impl State {
    pub(super) fn reviewing(&self) -> bool {
        self.review.is_some()
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn attach(&mut self, value: wasm_bindgen::JsValue) {
        self.service = Some(value);
    }
    fn cancel(&mut self) {
        if let Some(review) = &self.review
            && let Some(pending) = &review.pending
        {
            pending.cancelled.set(true);
            #[cfg(target_arch = "wasm32")]
            if let Some(ticket) = pending.cancellation.borrow().as_ref() {
                ticket.cancel();
            }
        }
        self.review = None;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
// This named variant remains denied until the separate production qualification.
pub(super) fn admit() -> Result<()> {
    use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ExportFlat,
        Family::Msp,
        Platform::Web,
    );
    request.extension = Some("msp");
    request.variant = Some(wire::EXPORT_VARIANT);
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
impl WebApp {
    fn retire_msp_export(&mut self) {
        self.msp.cancel();
        if self.export_confirm == Some(ImageExport::Msp) {
            self.export_confirm = None;
        }
    }
    pub(super) fn begin_msp_export(&mut self, ctx: &egui::Context) {
        let result: Result<()> = (|| {
            admit()?;
            if self.name.len() > 4096 {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "MSP2 filename allowance",
                ));
            }
            let owner = crate::importing::Owner::capture(self);
            let mut review = Review {
                owner,
                name: self.name.clone(),
                filename: self.download_name(Some(ImageExport::Msp)),
                version: self.msp_version,
                pending: None,
                prepared: None,
                texture: None,
                shown: None,
                approved: false,
                error: None,
            };
            if !review.current(self) || self.files.busy || self.jxl.busy() {
                return Err(stale());
            }
            self.launch_msp(&mut review, ctx);
            self.msp.review = Some(review);
            Ok(())
        })();
        if let Err(error) = result {
            self.message = error.to_string();
        }
    }
    fn launch_msp(&self, review: &mut Review, ctx: &egui::Context) {
        let result: Result<()> = (|| {
            let doc = self.editor.session.snapshot();
            let plan = wire::SourcePlan::capture(&doc)?;
            #[cfg(target_arch = "wasm32")]
            {
                let service = self
                    .msp
                    .service
                    .clone()
                    .ok_or_else(|| failed("Browser codec service unavailable"))?;
                if bridge::quarantined(&service) {
                    return Err(failed("Browser codec service requires a reload"));
                }
                let result = Rc::new(RefCell::new(None));
                let target = result.clone();
                let cancelled = Rc::new(Cell::new(false));
                let stop = cancelled.clone();
                let cancellation = Rc::new(RefCell::new(None));
                let token = cancellation.clone();
                let owner = review.owner.wire();
                let nonce = uuid::Uuid::new_v4();
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let value = if stop.get() {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "MSP2 export cancelled",
                        ))
                    } else {
                        bridge::prepare(service, doc, plan, owner, nonce, token).await
                    };
                    *target.borrow_mut() = Some(value);
                    ctx.request_repaint();
                });
                review.pending = Some(Pending {
                    version: review.version,
                    result,
                    cancelled,
                    cancellation,
                });
                Ok(())
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (doc, plan, ctx);
                Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Browser Worker is unavailable in host build",
                ))
            }
        })();
        if let Err(error) = result {
            review.error = Some(error);
        }
    }
    pub(crate) fn poll_msp_export(&mut self, ctx: &egui::Context) {
        let downloaded = self
            .msp
            .download
            .as_ref()
            .and_then(|(_, value)| value.borrow_mut().take());
        if let Some(result) = downloaded {
            let (owner, _) = self.msp.download.take().unwrap();
            if owner.current(self) {
                self.message = t!(
                    ctx,
                    if result.is_ok() {
                        "Download requested - verify your copy; this tab remains unsaved"
                    } else {
                        "MSP download could not be requested."
                    }
                );
            }
        }
        let Some(mut review) = self.msp.review.take() else {
            return;
        };
        if !review.current(self) {
            self.msp.review = Some(review);
            self.retire_msp_export();
            return;
        }
        if let Some(value) = review
            .pending
            .as_ref()
            .and_then(|p| p.result.borrow_mut().take())
        {
            let pending = review.pending.take().unwrap();
            if pending.version == review.version {
                match value {
                    Ok(prepared) => {
                        review.approved = false;
                        review.prepared = Some(prepared);
                    }
                    Err(error) => review.error = Some(error),
                }
            }
        }
        if review.pending.is_none() && review.prepared.is_none() && review.error.is_none() {
            self.launch_msp(&mut review, ctx);
        }
        if let Some(prepared) = &mut review.prepared
            && let Some(image) = prepared.preview.take()
        {
            review.texture =
                Some(ctx.load_texture("msp-browser-preview", image, egui::TextureOptions::NEAREST));
            review.shown = None;
        }
        if review.pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(10));
        }
        self.msp.review = Some(review);
    }
}
