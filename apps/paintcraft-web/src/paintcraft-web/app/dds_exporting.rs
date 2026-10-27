//! Opt-in advanced browser DDS uses the one host-installed isolated service.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::dds::{self, web_export as wire};
use paintcraft_ui::dds_export::{self as controls, Settings};
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
    #[cfg(any(target_arch = "wasm32", test))]
    fn bytes(&self) -> &[u8] {
        &self.response[self.encoded.clone()]
    }
}
type Outcome<T> = Rc<RefCell<Option<Result<T>>>>;
#[derive(Debug)]
struct Pending {
    settings: Settings,
    result: Outcome<Prepared>,
    cancelled: Rc<Cell<bool>>,
}
struct Review {
    owner: crate::importing::Owner,
    name: String,
    filename: String,
    settings: Settings,
    pending: Option<Pending>,
    prepared: Option<Prepared>,
    texture: Option<egui::TextureHandle>,
    shown: Option<u64>,
    approved: bool,
    error: Option<Error>,
}
impl std::fmt::Debug for Review {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DdsReview")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}
impl Review {
    fn current(&self, app: &WebApp) -> bool {
        self.owner.current(app)
            && self.name == app.name
            && app.frontend == Frontend::Studio
            && app.export_confirm == Some(ImageExport::Dds)
            && !app.paste.busy()
            && !app.editor.tools.is_constructing_shape()
            && app.editor.tools.floating_selection().is_none()
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
            if let Some(service) = &self.service {
                bridge::cancel(service);
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
pub(super) fn effective(mut settings: Settings, size: (u32, u32)) -> Result<Settings> {
    let count = settings.level_count(size.0, size.1).ok_or_else(stale)?;
    if count == 1 {
        settings.mips = controls::Mips::None;
        settings.filter = controls::Filter::Nearest;
    }
    if settings.encoding == controls::Encoding::Rgba8 {
        settings.quality = controls::Quality::Normal;
    }
    Ok(settings)
}
pub(super) fn advanced(settings: Settings, size: (u32, u32)) -> bool {
    effective(settings, size).is_ok_and(|s| s != Settings::default())
}
fn options(settings: Settings) -> dds::TextureOptions {
    dds::TextureOptions {
        encoding: match settings.encoding {
            controls::Encoding::Rgba8 => dds::Encoding::Rgba8,
            controls::Encoding::Bc1 => dds::Encoding::Bc1,
            controls::Encoding::Bc2 => dds::Encoding::Bc2,
            controls::Encoding::Bc3 => dds::Encoding::Bc3,
        },
        transfer: if settings.transfer == controls::Transfer::Srgb {
            dds::Transfer::Srgb
        } else {
            dds::Transfer::Linear
        },
        alpha: if settings.encoding == controls::Encoding::Bc1 {
            dds::SourceAlpha::Opaque
        } else {
            dds::SourceAlpha::Straight
        },
        mips: match settings.mips {
            controls::Mips::None => dds::MipCount::Levels(1),
            controls::Mips::Full => dds::MipCount::Full,
            controls::Mips::Custom(n) => dds::MipCount::Levels(n),
        },
        filter: if settings.filter == controls::Filter::Nearest {
            dds::MipFilter::Nearest
        } else {
            dds::MipFilter::BoxStored
        },
        quality: if settings.quality == controls::Quality::Normal {
            dds::TextureQuality::Normal
        } else {
            dds::TextureQuality::High
        },
    }
}
// This named variant remains denied until the separate production qualification.
pub(super) fn admit() -> Result<()> {
    use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ExportFlat,
        Family::Dds,
        Platform::Web,
    );
    request.extension = Some("dds");
    request.variant = Some("web-dds-texture-worker-v1");
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
impl WebApp {
    pub(super) fn begin_dds_export(&mut self, ctx: &egui::Context) {
        let result: Result<()> = (|| {
            admit()?;
            if self.name.len() > 4096 {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "DDS filename allowance",
                ));
            }
            let owner = crate::importing::Owner::capture(self);
            let mut review = Review {
                owner,
                name: self.name.clone(),
                filename: self.download_name(Some(ImageExport::Dds)),
                settings: effective(self.dds_settings, self.editor.session.document().canvas)?,
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
            self.launch_dds(&mut review, ctx);
            self.dds.review = Some(review);
            Ok(())
        })();
        if let Err(error) = result {
            self.message = error.to_string();
        }
    }
    fn launch_dds(&self, review: &mut Review, ctx: &egui::Context) {
        let result: Result<()> = (|| {
            let doc = self.editor.session.snapshot();
            let plan = wire::SourcePlan::capture(&doc, options(review.settings))?;
            #[cfg(target_arch = "wasm32")]
            {
                let service = self
                    .dds
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
                let owner = review.owner.wire();
                let nonce = uuid::Uuid::new_v4();
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let value = if stop.get() {
                        Err(Error::new(
                            ErrorCode::OperationCancelled,
                            "DDS export cancelled",
                        ))
                    } else {
                        bridge::prepare(service, doc, plan, owner, nonce).await
                    };
                    *target.borrow_mut() = Some(value);
                    ctx.request_repaint();
                });
                review.pending = Some(Pending {
                    settings: review.settings,
                    result,
                    cancelled,
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
    pub(crate) fn poll_dds_export(&mut self, ctx: &egui::Context) {
        let downloaded = self
            .dds
            .download
            .as_ref()
            .and_then(|(_, value)| value.borrow_mut().take());
        if let Some(result) = downloaded {
            let (owner, _) = self.dds.download.take().unwrap();
            if owner.current(self) {
                self.message = t!(
                    ctx,
                    if result.is_ok() {
                        "Download requested - verify your copy; this tab remains unsaved"
                    } else {
                        "DDS download could not be requested."
                    }
                );
            }
        }
        let Some(mut review) = self.dds.review.take() else {
            return;
        };
        if !review.current(self) {
            self.dds.review = Some(review);
            self.dds.cancel();
            if self.export_confirm == Some(ImageExport::Dds) {
                self.export_confirm = None;
            }
            return;
        }
        if let Some(value) = review
            .pending
            .as_ref()
            .and_then(|p| p.result.borrow_mut().take())
        {
            let pending = review.pending.take().unwrap();
            if pending.settings == review.settings {
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
            self.launch_dds(&mut review, ctx);
        }
        if let Some(prepared) = &mut review.prepared
            && let Some(image) = prepared.preview.take()
        {
            review.texture =
                Some(ctx.load_texture("dds-browser-preview", image, egui::TextureOptions::NEAREST));
            review.shown = None;
        }
        if review.pending.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(10));
        }
        self.dds.review = Some(review);
    }
}
