//! Browser File-owned isolated import. Bytes remain private until full owner and
//! wire validation, resident tile admission and a guarded new-session install.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::codec_worker as wire;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
mod bridge;
mod response;
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub name: String,
    pub length: u64,
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
            || size > wire::INPUT_BYTES_MAX as f64
            || name.len() > 4096
        {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "JPEG XL source exceeds browser allowance",
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
struct Prepared {
    document: Document,
    name: String,
    #[cfg(target_arch = "wasm32")]
    lease: bridge::Lease,
}
#[derive(Debug)]
struct Pending {
    owner: Owner,
    cancelled: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    cancellation: Rc<RefCell<Option<bridge::Cancellation>>>,
    result: Rc<RefCell<Option<Result<Prepared>>>>,
}
#[derive(Debug, Default)]
pub(crate) struct State {
    pending: Option<Pending>,
    quarantined: bool,
    #[cfg(target_arch = "wasm32")]
    service: Option<wasm_bindgen::JsValue>,
}
impl State {
    pub(crate) fn busy(&self) -> bool {
        self.pending.is_some()
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn attach(&mut self, service: wasm_bindgen::JsValue) {
        self.service = Some(service);
    }
    fn cancel(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancelled.set(true);
            #[cfg(target_arch = "wasm32")]
            if let Some(ticket) = pending.cancellation.borrow().as_ref() {
                ticket.cancel();
            }
        }
    }
    fn refresh(&mut self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(service) = &self.service {
            self.quarantined |= bridge::quarantined(service).unwrap_or(true);
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
    pub(crate) fn accept_jxl(&mut self, source: Source, ctx: &eframe::egui::Context) {
        let Some(owner) = self.take_jxl_owner() else {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        };
        self.jxl.refresh();
        if self.jxl.quarantined {
            self.message = t!(ctx, PAUSED);
            return;
        }
        if self.format_profile() != paintcraft_formats::registry::Frontend::Studio
            || self.jxl.pending.is_some()
        {
            self.message = t!(ctx, "The document changed; choose the image again.");
            return;
        }
        if let Err(error) = admit(&source.name) {
            self.message = error.to_string();
            return;
        }
        let Some(service) = self.jxl.service.clone() else {
            self.message = t!(ctx, FAILED);
            return;
        };
        if bridge::sync_owner(&service, owner.wire()).is_err() {
            self.message = t!(ctx, FAILED);
            return;
        }
        let cancelled = Rc::new(Cell::new(false));
        let result = Rc::new(RefCell::new(None));
        let target = result.clone();
        let stop = cancelled.clone();
        let cancellation = Rc::new(RefCell::new(None));
        let token = cancellation.clone();
        let wire_owner = owner.wire();
        let nonce = Uuid::new_v4();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let value = bridge::prepare(&service, source, wire_owner, nonce, stop, token).await;
            *target.borrow_mut() = Some(value);
            ctx.request_repaint();
        });
        self.jxl.pending = Some(Pending {
            owner,
            cancelled,
            cancellation,
            result,
        });
    }
    fn take_jxl_owner(&mut self) -> Option<Owner> {
        if self.blocked() {
            return None;
        }
        self.import_owner.take().filter(|owner| owner.current(self))
    }
    pub(crate) fn poll_jxl(&mut self, ctx: &eframe::egui::Context) {
        if self
            .jxl
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.owner.current(self))
        {
            self.jxl.cancel();
        }
        let value = self
            .jxl
            .pending
            .as_ref()
            .and_then(|pending| pending.result.borrow_mut().take());
        self.jxl.refresh();
        let Some(value) = value else {
            if self.jxl.busy() {
                ctx.request_repaint_after(std::time::Duration::from_millis(10));
            }
            return;
        };
        let pending = self.jxl.pending.take().expect("result has owner");
        match value {
            Ok(prepared) if pending.owner.current(self) && !pending.cancelled.get() => {
                // Tiles already own the existing realm's resident leases. Build
                // the new Editor privately before retiring the JS preparation.
                let Prepared {
                    document,
                    name,
                    #[cfg(target_arch = "wasm32")]
                    mut lease,
                } = prepared;
                let editor = crate::editor::Editor::from_document(document);
                #[cfg(target_arch = "wasm32")]
                let released = lease.release();
                #[cfg(not(target_arch = "wasm32"))]
                let released = Ok::<_, Error>(());
                match (editor, released) {
                    (Ok(mut editor), Ok(())) if pending.owner.current(self) => {
                        editor.session.mark_unsaved();
                        self.install_import_editor(editor, name);
                        self.message = t!(ctx, "JPEG XL imported as an editable project copy.");
                    }
                    _ => self.message = t!(ctx, FAILED),
                }
            }
            Ok(_) => self.message = t!(ctx, "The document changed; choose the image again."),
            Err(error) => {
                self.message = if self.jxl.quarantined {
                    t!(ctx, PAUSED)
                } else if error.code == ErrorCode::OperationCancelled {
                    t!(ctx, "Import cancelled.")
                } else {
                    t!(ctx, FAILED)
                }
            }
        }
    }
    pub(crate) fn jxl_panel(&mut self, ui: &mut eframe::egui::Ui) {
        self.jxl.refresh();
        if self.jxl.quarantined {
            eframe::egui::Panel::top("web-jxl-quarantine").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(t!(ui.ctx(), PAUSED));
                    if ui.button(t!(ui.ctx(), "Download project")).clicked() {
                        self.download(ui.ctx(), false);
                    }
                });
            });
        }
    }
    pub(crate) fn jxl_dialog(&mut self, ctx: &eframe::egui::Context) -> bool {
        if !self.jxl.busy() {
            return false;
        }
        let mut cancel = false;
        eframe::egui::Modal::new(eframe::egui::Id::unique("web-jxl-progress")).show(ctx, |ui| {
            ui.heading(t!(ui.ctx(), "Importing JPEG XL"));
            ui.spinner();
            cancel = ui.button(t!(ui.ctx(), "Cancel")).clicked();
        });
        if cancel || ctx.input(|input| input.key_pressed(eframe::egui::Key::Escape)) {
            self.jxl.cancel();
        }
        true
    }
}
const PAUSED: &str = "JPEG XL importing is paused for this tab. Download your project, then reload this tab to try again.";
const FAILED: &str = "JPEG XL import failed; the current document was kept.";
pub(crate) const VARIANT: &str = paintcraft_formats::registry::WEB_JXL_IMPORT_VARIANT;
pub(crate) fn admit(name: &str) -> Result<()> {
    use paintcraft_formats::registry::{self, Family, Frontend, Operation, Platform, Request};
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        Family::Jxl,
        Platform::Web,
    );
    request.variant = Some(VARIANT);
    request.extension = name.rsplit('.').next();
    let decision = registry::admission(request);
    if decision.available() {
        Ok(())
    } else {
        decision.require_allowed(request)
    }
}
#[cfg(test)]
mod tests;
