//! One frozen background action borrows the display's SINGLE admitted slot.
use super::*;
mod flatten;
#[cfg(test)]
mod flatten_tests;
mod picker;
#[cfg(test)]
mod picker_tests;
use paintcraft_core::types::{
    Rect, Result,
    json::Value,
    resources::{AllocationClass, BudgetAccount},
};
use paintcraft_document::Document;
use paintcraft_editor::session::HistoryTicket;
use paintcraft_raster::selection::{BOUNDS_CACHE_WORK, Selection};
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
#[derive(Clone)]
struct Frozen {
    owner: Uuid,
    ticket: HistoryTicket,
    base: Arc<Document>,
    preview: paintcraft_tools::PreviewRevision,
    fonts: u64,
    paste: Option<paintcraft_tools::PreviewRevision>,
    frontend: Frontend,
    variant: ClassicVariant,
}
impl Frozen {
    fn capture(app: &WebApp) -> Self {
        Self {
            owner: app.editor.session.owner_id(),
            ticket: app.editor.session.history_ticket(),
            base: app.editor.session.snapshot(),
            preview: app.editor.tools.preview_revision(),
            fonts: app.text.font_generation,
            paste: app.paste_revision(),
            frontend: app.frontend,
            variant: app.variant,
        }
    }
    fn current(&self, app: &WebApp) -> bool {
        self.owner == app.editor.session.owner_id()
            && self.ticket == app.editor.session.history_ticket()
            && Arc::ptr_eq(&self.base, &app.editor.session.snapshot())
            && self.preview == app.editor.tools.preview_revision()
            && self.fonts == app.text.font_generation
            && self.paste == app.paste_revision()
            && app.filter_editing.is_none()
            && self.frontend == app.frontend
            && self.variant == app.variant
            && !app.text.is_composing()
    }
}
struct Mask {
    bytes: Vec<u8>,
    _account: BudgetAccount,
}
enum Purpose {
    Download { name: String },
    Print { title: String },
    Clipboard(crate::clipboard::background::Deferred),
    Picker(picker::Intent),
    Flatten,
}
struct Encoded {
    bytes: Vec<u8>,
    mime: String,
}
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
enum Output {
    Encoded(Encoded),
    Picked(paintcraft_core::types::Rgba),
    Flattened(flatten::Pixels),
}
type Outcome = Rc<RefCell<Option<Result<Output>>>>;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(super) struct Job {
    frozen: Frozen,
    flatten: Option<flatten::Transaction>,
    capture: Capture,
    region: Rect,
    spec: Value,
    mask: Option<Mask>,
    purpose: Option<Purpose>,
    outcome: Option<Outcome>,
    cancelled: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    control: Rc<RefCell<Option<bridge::CopyControl>>>,
}
fn rejected(message: &str) -> paintcraft_core::types::Error {
    paintcraft_core::types::Error::new(
        paintcraft_core::types::ErrorCode::CapabilityUnsupported,
        message,
    )
}
fn digest(bytes: &[u8]) -> String {
    paintcraft_core::types::policy::hex(&Sha256::digest(bytes))
}
impl WebApp {
    fn queue_flat_action(
        &mut self,
        ctx: &egui::Context,
        doc: Arc<Document>,
        purpose: Purpose,
        region: Rect,
        output: (Value, Mask),
    ) -> Result<()> {
        let (spec, mask) = output;
        if self.filter_display.copy_action.is_some() || self.filter_display.picker_wanted.is_some()
        {
            return Err(rejected("Another background image action is pending"));
        }
        if !render::stored_scoped(&doc) {
            return Err(rejected("Background image action requires stored Noise"));
        }
        if let Purpose::Download { name } | Purpose::Print { title: name } = &purpose
            && name.len() > 1024
        {
            return Err(rejected("Background image name exceeds1024 bytes"));
        }
        // File/print preparation may just have ended a stroke. Its old private
        // texture cannot masquerade as a valid pending BASE during suspension.
        // Keep a still-active text/IME preview honest and resumable instead.
        if !self.ordinary_filter_preview() {
            self.forget_noise_preview();
        }
        let frozen = Frozen::capture(self);
        let mut capture = self.filter_capture();
        capture.ticket.state_id = doc.state_id;
        capture.document = doc;
        capture.scoped = true;
        capture.programs = true;
        self.filter_display.copy_action = Some(Job {
            frozen,
            flatten: None,
            capture,
            region,
            spec,
            mask: Some(mask),
            purpose: Some(purpose),
            outcome: None,
            cancelled: Rc::new(Cell::new(false)),
            #[cfg(target_arch = "wasm32")]
            control: Rc::default(),
        });
        ctx.request_repaint();
        Ok(())
    }
    fn empty_copy_mask(&self) -> Mask {
        Mask {
            bytes: Vec::new(),
            _account: self.clipboard.budget.account(AllocationClass::SharedBudget),
        }
    }
    pub(crate) fn queue_noise_download(
        &mut self,
        ctx: &egui::Context,
        doc: Arc<Document>,
        options: crate::files::ExportOptions,
        name: String,
    ) -> Result<()> {
        use paintcraft_formats::export::{filter_bake, metadata_policy};
        use paintcraft_formats::registry::{Family, Operation};
        let (format, family) = match options.format {
            Some(ImageExport::Png) => ("png", Family::Png),
            Some(ImageExport::Jpeg) => ("jpeg", Family::Jpeg),
            Some(ImageExport::Bmp) => ("bmp", Family::Bmp),
            _ => {
                return Err(rejected(
                    "This image codec has no bounded background Noise adapter",
                ));
            }
        };
        let target = metadata_policy::Target::new(family, Operation::ExportFlat);
        metadata_policy::Policy::capture(&doc, target, options.source_metadata)?
            .validate(&doc, target)?;
        options.filter_bake.validate(&doc, target, &|| Ok(()))?;
        let omit = matches!(options.source_metadata, metadata_policy::Intent::Omit);
        let bake = matches!(options.filter_bake, filter_bake::Policy::Bake(_));
        let spec = spec(
            "download",
            format,
            options.jpeg_quality,
            options.jpeg_matte.unwrap_or_default().rgb(),
            options.frontend.id(),
            (omit, bake),
            &[],
        );
        self.queue_flat_action(
            ctx,
            doc.clone(),
            Purpose::Download { name },
            doc.canvas_rect(),
            (spec, self.empty_copy_mask()),
        )
    }
    pub(crate) fn queue_noise_print(
        &mut self,
        ctx: &egui::Context,
        doc: Arc<Document>,
        title: String,
    ) -> Result<()> {
        let spec = spec("print", "png", 90, [255; 3], "studio", (false, false), &[]);
        self.queue_flat_action(
            ctx,
            doc.clone(),
            Purpose::Print { title },
            doc.canvas_rect(),
            (spec, self.empty_copy_mask()),
        )
    }
    pub(crate) fn queue_noise_visible_copy(
        &mut self,
        ctx: &egui::Context,
        doc: Arc<Document>,
        selection: Selection,
    ) -> Result<()> {
        if self.filter_display.copy_action.is_some() || self.filter_display.picker_wanted.is_some()
        {
            return Err(rejected("Another background image action is pending"));
        }
        // Consume activation before selection bounds/capture or async awaits.
        // Any subsequent validation/allocation failure drops and rejects it.
        let deferred = crate::clipboard::background::Deferred::begin()?;
        let bounds = match &selection {
            Selection::Inactive => Some(doc.canvas_rect()),
            Selection::Empty => None,
            Selection::Rect(rect) => rect.intersect(&doc.canvas_rect()),
            Selection::Mask(mask) => {
                mask.bounds_checked(doc.canvas_rect(), BOUNDS_CACHE_WORK, &|| Ok(()))?
            }
        }
        .ok_or_else(|| rejected("There are no selected pixels to copy"))?;
        let count = (bounds.width() as u64)
            .checked_mul(bounds.height() as u64)
            .ok_or_else(|| rejected("Clipboard extent"))?;
        if count > 4_194_304 {
            return Err(rejected("Clipboard exceeds4MP"));
        }
        let mut mask = self.empty_copy_mask();
        if let Selection::Mask(field) = selection {
            mask._account.resize(count)?;
            mask.bytes
                .try_reserve_exact(count as usize)
                .map_err(|_| rejected("Clipboard mask allocation"))?;
            if mask.bytes.capacity() as u64 > 4_194_304 {
                return Err(rejected("Clipboard mask capacity"));
            }
            mask._account.resize(mask.bytes.capacity() as u64)?;
            for y in bounds.y0..bounds.y1 {
                for x in bounds.x0..bounds.x1 {
                    mask.bytes
                        .push(if field.coverage(x, y) == 0 { 0 } else { 255 });
                }
            }
        }
        let spec = spec(
            "clipboard",
            "png",
            90,
            [255; 3],
            "studio",
            (false, false),
            &mask.bytes,
        );
        self.queue_flat_action(ctx, doc, Purpose::Clipboard(deferred), bounds, (spec, mask))
    }
    pub(crate) fn cancel_background_image(&mut self) {
        self.filter_display.picker_wanted = None;
        if let Some(job) = &self.filter_display.copy_action {
            job.cancelled.set(true);
            #[cfg(target_arch = "wasm32")]
            if let Some(control) = job.control.borrow().as_ref() {
                control.cancel();
            }
        }
    }
    pub(super) fn pump_flat_action(&mut self, ctx: &egui::Context) -> bool {
        if self.filter_display.copy_action.is_none() {
            self.filter_display.copy_action = self.filter_display.picker_wanted.take();
        }
        let busy = self.pump_background_action(ctx);
        // Do not resume display between closing an old picker and its latest
        // replacement. Both use the existing single admitted slot.
        busy || self.filter_display.picker_wanted.is_some()
    }
    fn pump_background_action(&mut self, ctx: &egui::Context) -> bool {
        if self.filter_display.copy_action.is_some() {
            let cancel =
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            let button = egui::Area::new(egui::Id::unique("background-image-progress"))
                .anchor(egui::Align2::CENTER_TOP, [0.0, 16.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.button(paintcraft_ui::t!(ui.ctx(), "Cancel")).clicked()
                    })
                    .inner
                })
                .inner;
            if cancel || button {
                self.cancel_background_image();
            }
        }
        let Some(mut job) = self.filter_display.copy_action.take() else {
            return false;
        };
        if !job.current(self) {
            job.cancelled.set(true);
            #[cfg(target_arch = "wasm32")]
            if let Some(control) = job.control.borrow().as_ref() {
                control.cancel();
            }
        }
        if job.outcome.is_none() {
            match self.filter_display.renderer.suspend_for_copy(ctx) {
                Ok(false) => {
                    self.filter_display.copy_action = Some(job);
                    return true;
                }
                Err(error) => {
                    self.filter_display.picker_wanted = None;
                    self.message = error.to_string();
                    return false;
                }
                Ok(true) => {}
            }
            if job.cancelled.get() {
                self.message = "Background image action cancelled".into();
                ctx.request_repaint();
                return false;
            }
            let outcome: Outcome = Rc::default();
            job.outcome = Some(outcome.clone());
            #[cfg(target_arch = "wasm32")]
            {
                let capture = job.capture.clone();
                let region = job.region;
                let spec = job.spec.clone();
                let mask = job.mask.take().expect("pending mask");
                let cancelled = job.cancelled.clone();
                let control = job.control.clone();
                let is_picker = matches!(job.purpose, Some(Purpose::Picker(_)));
                let raw_account = job.flatten.as_mut().and_then(|t| t.account.take());
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let slot = Rc::default();
                    let result = match bridge::open(capture.clone(), slot).await {
                        Ok(client) => {
                            *control.borrow_mut() = Some(client.copy_control());
                            let encoded = if cancelled.get() {
                                Err(rejected("Background action cancelled"))
                            } else if let Some(account) = raw_account {
                                flatten::render(&client, &capture, region, account)
                                    .await
                                    .map(Output::Flattened)
                            } else if is_picker {
                                picker::sample(&client, &capture, region)
                                    .await
                                    .map(Output::Picked)
                            } else {
                                client
                                    .encode_copy(region, &spec, &mask.bytes)
                                    .await
                                    .map(|e| {
                                        Output::Encoded(Encoded {
                                            bytes: e.bytes,
                                            mime: e.mime,
                                        })
                                    })
                            };
                            // Even a stale/error result cannot hand the slot back
                            // before acknowledged source close.
                            let closed = client.close().await;
                            control.borrow_mut().take();
                            match closed {
                                Ok(()) => encoded,
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(error),
                    };
                    *outcome.borrow_mut() = Some(result);
                    ctx.request_repaint();
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                *outcome.borrow_mut() =
                    Some(Err(rejected("Background image Worker requires browser")));
            }
        }
        let result = job
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.borrow_mut().take());
        let Some(result) = result else {
            self.filter_display.copy_action = Some(job);
            return true;
        };
        if job.cancelled.get() || !job.current(self) {
            self.message = "Background image action cancelled because its source changed".into();
        } else {
            match result {
                Ok(output) => match (job.purpose.take().expect("pending purpose"), output) {
                    (Purpose::Download { name }, Output::Encoded(encoded)) => {
                        self.files.download_asset_as(
                            ctx,
                            &encoded.bytes,
                            &name,
                            &encoded.mime,
                            job.frozen.owner,
                        )
                    }
                    (Purpose::Print { title }, Output::Encoded(encoded)) => {
                        self.printer.request_encoded(ctx, encoded.bytes, title)
                    }
                    (Purpose::Clipboard(deferred), Output::Encoded(encoded)) => {
                        match deferred.complete(ctx, &encoded.bytes) {
                            Ok(ack) => self.clipboard.accept_background(ack),
                            Err(error) => self.message = error.to_string(),
                        }
                    }
                    (Purpose::Picker(intent), Output::Picked(color)) => {
                        match self.editor.apply_sampled_color(&intent.pick, color) {
                            Ok(()) => {
                                self.controls.primary = self.editor.tools.color;
                                self.controls.secondary = self.editor.tools.secondary;
                                self.controls.tool = self.editor.tools.tool;
                            }
                            Err(error) => self.message = error.to_string(),
                        }
                    }
                    (Purpose::Flatten, Output::Flattened(pixels)) => {
                        match flatten::publish(
                            self,
                            job.flatten.take().expect("flatten transaction"),
                            pixels,
                        ) {
                            Ok(()) => {
                                self.message = if self.unsaved() {
                                    "Unsaved changes - kept only in this tab".into()
                                } else {
                                    "Ready".into()
                                };
                                self.update_title();
                            }
                            Err(error) => self.message = format!("Edit failed: {error}"),
                        }
                    }
                    _ => self.message = "Background result kind changed".into(),
                },
                Err(error) => self.message = format!("Background image unavailable: {error}"),
            }
        }
        // Existing BASE texture was retained throughout; the normal Worker path
        // resumes on this pass and independently validates its source/view.
        self.canvas.invalidate();
        ctx.request_repaint();
        false
    }
}
fn spec(
    purpose: &str,
    format: &str,
    quality: u8,
    matte: [u8; 3],
    frontend: &str,
    consent: (bool, bool),
    mask: &[u8],
) -> Value {
    Value::object([
        ("version", Value::Int(1)),
        ("purpose", Value::str(purpose)),
        ("format", Value::str(format)),
        ("quality", Value::Int(i64::from(quality))),
        (
            "matte",
            Value::Array(
                matte
                    .into_iter()
                    .map(|v| Value::Int(i64::from(v)))
                    .collect(),
            ),
        ),
        ("frontend", Value::str(frontend)),
        ("omitMetadata", Value::Bool(consent.0)),
        ("bakeFilters", Value::Bool(consent.1)),
        ("maskLength", Value::Int(mask.len() as i64)),
        ("maskSha256", Value::str(digest(mask))),
    ])
}
