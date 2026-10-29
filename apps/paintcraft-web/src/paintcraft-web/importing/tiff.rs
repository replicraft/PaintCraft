//! Explicit page selection retains the immutable browser File and captured owner.
use super::*;
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{AllocationClass, BudgetAccount},
};
#[cfg(target_arch = "wasm32")]
use paintcraft_formats::codec_worker as decoder;
use paintcraft_formats::tiff_pages::{self, browser_wire as wire};
use paintcraft_ui::page_import::{self, PageChoice, Response};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[cfg(target_arch = "wasm32")]
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
mod bridge;
mod response;
mod review;
#[derive(Clone, Debug)]
pub(crate) struct Source {
    name: String,
    length: u64,
    #[cfg(target_arch = "wasm32")]
    file: web_sys::File,
}
impl Source {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(file: web_sys::File) -> Result<Self> {
        let size = file.size();
        let name = file.name();
        if !size.is_finite()
            || size.fract() != 0.0
            || size < 1.0
            || size > wire::SOURCE_MAX as f64
            || name.len() > 4096
            || name
                .rsplit('.')
                .next()
                .and_then(paintcraft_formats::registry::Family::from_extension)
                != Some(paintcraft_formats::registry::Family::Tiff)
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "TIFF browser source allowance",
            ));
        }
        Ok(Self {
            name,
            length: size as u64,
            file,
        })
    }
}
#[derive(Debug)]
struct Held {
    source: Source,
    _metadata: BudgetAccount,
}
impl Held {
    fn new(source: Source) -> Result<Self> {
        let mut metadata = paintcraft_raster::Admission::new(0)
            .residency_budget()
            .account(AllocationClass::PaintMetadata);
        // Retained File and bounded chooser metadata have their own declaration;
        // the transient Worker lease is retired before a selected-page job starts.
        metadata.resize(source.length + u64::from(wire::PAGES_MAX) * 128 + 65536)?;
        Ok(Self {
            source,
            _metadata: metadata,
        })
    }
}
#[derive(Debug)]
struct Prepared {
    document: Document,
    report: tiff_pages::Assessment,
    selection: wire::Selection,
    #[cfg(target_arch = "wasm32")]
    lease: bridge::Lease,
}
#[derive(Debug)]
enum Output {
    Inventory {
        pages: Vec<wire::Page>,
        source_sha256: [u8; 32],
    },
    Decoded(Box<Prepared>),
}
#[derive(Debug)]
struct Pending {
    held: Rc<Held>,
    owner: Owner,
    cancelled: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    cancellation: Rc<RefCell<Option<bridge::Cancellation>>>,
    result: Rc<RefCell<Option<Result<Output>>>>,
}
#[derive(Debug)]
struct Chooser {
    held: Rc<Held>,
    owner: Owner,
    pages: Vec<wire::Page>,
    choices: Vec<PageChoice>,
    source_sha256: [u8; 32],
    selected: Option<u16>,
}
#[derive(Debug, Default)]
pub(crate) struct State {
    pending: Option<Pending>,
    chooser: Option<Chooser>,
    review: Option<review::Review>,
    pub(crate) quarantined: bool,
    #[cfg(target_arch = "wasm32")]
    service: Option<wasm_bindgen::JsValue>,
}
impl State {
    pub(crate) fn busy(&self) -> bool {
        self.pending.is_some() || self.chooser.is_some() || self.review.is_some()
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn attach(&mut self, service: wasm_bindgen::JsValue) {
        self.service = Some(service);
    }
    fn cancel(&mut self) {
        self.chooser = None;
        self.review = None;
        if let Some(pending) = &self.pending {
            pending.cancelled.set(true);
            #[cfg(target_arch = "wasm32")]
            if let Some(ticket) = pending.cancellation.borrow().as_ref() {
                ticket.cancel();
            }
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl WebApp {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn accept_tiff(&mut self, source: Source, ctx: &eframe::egui::Context) {
        let Some(owner) = self.import_owner.take().filter(|owner| owner.current(self)) else {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        };
        if self.blocked()
            || self.format_profile() != paintcraft_formats::registry::Frontend::Studio
            || !admit(&source.name)
        {
            self.message = t!(ctx, FAILED);
            return;
        }
        match Held::new(source) {
            Ok(held) => self.start_tiff(Rc::new(held), owner, None, ctx),
            Err(_) => self.message = t!(ctx, FAILED),
        }
    }
    #[cfg(target_arch = "wasm32")]
    fn start_tiff(
        &mut self,
        held: Rc<Held>,
        owner: Owner,
        selection: Option<wire::Selection>,
        ctx: &eframe::egui::Context,
    ) {
        let Some(service) = self.tiff.service.clone() else {
            self.message = t!(ctx, FAILED);
            return;
        };
        if bridge::quarantined(&service).unwrap_or(true) {
            self.tiff.quarantined = true;
            self.message = t!(ctx, PAUSED);
            return;
        }
        if !owner.current(self) || bridge::sync_owner(&service, owner.wire()).is_err() {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        let cancelled = Rc::new(Cell::new(false));
        let cancellation = Rc::new(RefCell::new(None));
        let result = Rc::new(RefCell::new(None));
        let target = result.clone();
        let stop = cancelled.clone();
        let token = cancellation.clone();
        let captured = owner.wire();
        let nonce = Uuid::new_v4();
        let source = held.source.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let value =
                bridge::prepare(&service, source, captured, nonce, stop, token, selection).await;
            *target.borrow_mut() = Some(value);
            ctx.request_repaint();
        });
        self.tiff.pending = Some(Pending {
            held,
            owner,
            cancelled,
            cancellation,
            result,
        });
    }
    pub(crate) fn poll_tiff(&mut self, ctx: &eframe::egui::Context) {
        if self
            .tiff
            .chooser
            .as_ref()
            .is_some_and(|value| !value.owner.current(self))
            || self
                .tiff
                .review
                .as_ref()
                .is_some_and(|value| !value.owner.current(self))
        {
            self.tiff.chooser = None;
            self.tiff.review = None;
            self.message = t!(ctx, "The document changed; choose the image again.");
        }
        if self
            .tiff
            .pending
            .as_ref()
            .is_some_and(|value| !value.owner.current(self))
        {
            self.tiff.cancel();
        }
        let value = self
            .tiff
            .pending
            .as_ref()
            .and_then(|pending| pending.result.borrow_mut().take());
        let Some(value) = value else {
            if self.tiff.pending.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(10));
            }
            return;
        };
        #[cfg(target_arch = "wasm32")]
        if let Some(service) = &self.tiff.service {
            self.tiff.quarantined |= bridge::quarantined(service).unwrap_or(true);
        }
        let pending = self.tiff.pending.take().expect("TIFF result retains owner");
        if !pending.owner.current(self) || pending.cancelled.get() {
            self.message = t!(ctx, "Import cancelled.");
            return;
        }
        match value {
            Ok(Output::Inventory {
                pages,
                source_sha256,
            }) => {
                let choices = pages
                    .iter()
                    .map(|page| PageChoice {
                        page: page_import::Page {
                            index: page.index,
                            width: page.width,
                            height: page.height,
                        },
                        supported: page.supported,
                    })
                    .collect();
                self.tiff.chooser = Some(Chooser {
                    held: pending.held,
                    owner: pending.owner,
                    pages,
                    choices,
                    source_sha256,
                    selected: None,
                });
            }
            Ok(Output::Decoded(prepared)) => {
                let prepared = *prepared;
                if prepared.report.losses().is_empty() {
                    self.install_tiff(pending.held, pending.owner, prepared, None, ctx);
                } else {
                    match review::Review::new(ctx, pending.held, pending.owner, prepared) {
                        Ok(review) => self.tiff.review = Some(review),
                        Err(_) => self.message = t!(ctx, FAILED),
                    }
                }
            }
            Err(error) => {
                self.message = if self.tiff.quarantined {
                    t!(ctx, PAUSED)
                } else if error.code == ErrorCode::OperationCancelled {
                    t!(ctx, "Import cancelled.")
                } else {
                    t!(ctx, FAILED)
                }
            }
        }
    }
    pub(crate) fn tiff_dialog(&mut self, ctx: &eframe::egui::Context) -> bool {
        if self.tiff.review.is_some() {
            self.tiff_review_dialog(ctx);
            return true;
        }
        if let Some(mut chooser) = self.tiff.chooser.take() {
            if !chooser.owner.current(self) {
                self.message = t!(ctx, "The document changed; choose the image again.");
                return false;
            }
            let response = page_import::dialog_profiled(
                ctx,
                page_import::Format::Tiff,
                &chooser.held.source.name,
                &chooser.choices,
                &mut chooser.selected,
                crate::editor::MAX_PIXELS,
                None,
            );
            match response {
                Response::Pending => self.tiff.chooser = Some(chooser),
                Response::Cancel => self.message = t!(ctx, "Import cancelled."),
                Response::Import(index) => {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(page) = chooser
                        .pages
                        .get(index as usize)
                        .copied()
                        .filter(|page| page.index == index && page.supported)
                    {
                        let selection = wire::Selection {
                            page,
                            page_count: chooser.pages.len() as u16,
                            source_sha256: chooser.source_sha256,
                        };
                        self.start_tiff(chooser.held, chooser.owner, Some(selection), ctx);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    let _ = index;
                }
            }
            return true;
        }
        if self.tiff.pending.is_none() {
            return false;
        }
        let mut cancel = false;
        eframe::egui::Modal::new(eframe::egui::Id::unique("web-tiff-progress")).show(ctx, |ui| {
            ui.heading(t!(ui.ctx(), "Import image"));
            ui.spinner();
            cancel = ui.button(t!(ui.ctx(), "Cancel")).clicked();
        });
        if cancel || ctx.input(|input| input.key_pressed(eframe::egui::Key::Escape)) {
            self.tiff.cancel();
        }
        true
    }
}
pub(crate) const PAUSED: &str = "Image importing is paused for this tab. Download your project, then reload this tab to try again.";
const FAILED: &str = "Image import failed; the current document was kept.";
fn admit(name: &str) -> bool {
    use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        Family::Tiff,
        Platform::Web,
    );
    request.extension = name.rsplit('.').next();
    request.variant = Some(tiff_pages::IMPORT_VARIANT);
    registry::admission(request).available()
}
