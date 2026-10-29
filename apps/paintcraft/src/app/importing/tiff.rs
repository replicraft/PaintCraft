//! Explicit Studio TIFF pages retain source, owner and selected preview consent.
use super::*;
use controller::background::{TiffCompletion, TiffPending};
use paintcraft_core::types::resources::{BudgetAccount, SharedBudget};
use paintcraft_formats::{
    registry::{self, Family, Operation, Platform, Request},
    tiff_pages as pages,
};
use paintcraft_platform::source::SourceIdentity;
use paintcraft_ui::page_import::{self, PageChoice, Response};
#[path = "tiff/review.rs"]
mod review;
#[path = "tiff/worker.rs"]
mod worker;

pub(crate) struct Frozen {
    bytes: Vec<u8>,
    inventory: pages::Inventory,
    choices: Vec<PageChoice>,
    identity: SourceIdentity,
    _account: BudgetAccount,
}
impl Frozen {
    fn verify(&self, path: &Path, check: &dyn Fn() -> Result<()>) -> Result<()> {
        self.identity.verify_bytes(
            path,
            self.inventory.source_bytes(),
            &self.inventory.source_sha256(),
            check,
        )
    }
}
pub(crate) struct Prepared {
    frozen: Arc<Frozen>,
    document: paintcraft_document::Document,
    assessment: pages::Assessment,
    losses: Vec<String>,
    thumbnail: Vec<u8>,
    thumbnail_size: [usize; 2],
    preview_budget: SharedBudget,
    _account: BudgetAccount,
}
pub(super) struct Source {
    stamp: controller::PreviewStamp,
    job: Option<TiffPending>,
    frozen: Option<Arc<Frozen>>,
    selected: Option<u16>,
    prepared: Option<Box<Prepared>>,
    review: Option<review::Review>,
    error: Option<String>,
}
fn admit(path: &Path) -> Result<()> {
    let mut request = Request::new(
        Frontend::Studio,
        Operation::ImportConvert,
        Family::Tiff,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|ext| ext.to_str());
    request.variant = Some(pages::IMPORT_VARIANT);
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
impl App {
    pub(crate) fn try_import_tiff_source(&mut self, path: &Path, convert: bool) -> Result<bool> {
        if !convert
            || self.format_frontend() != Frontend::Studio
            || path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Family::from_extension)
                != Some(Family::Tiff)
        {
            return Ok(false);
        }
        self.stage_tiff_import(path.to_path_buf())?;
        Ok(true)
    }
    pub(crate) fn stage_tiff_import(&mut self, path: PathBuf) -> Result<()> {
        if self.format_frontend() != Frontend::Studio {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "TIFF page import requires Studio",
            ));
        }
        admit(&path)?;
        if self.pending_import.is_some()
            || self.has_document_drafts()
            || self.editor.has_pending_edits()
            || self.editor.has_floating_selection()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current editing operation before importing a picture",
            ));
        }
        self.guard_asset_adoption(&path, None)?;
        let identity = SourceIdentity::existing_destination(&path)?.ok_or_else(|| {
            Error::new(
                ErrorCode::StorageFailed,
                "TIFF input must be a regular file",
            )
        })?;
        let length = identity
            .file()
            .metadata()
            .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))?
            .len();
        if length > pages::BYTES_MAX as u64 {
            return Err(Error::new(
                ErrorCode::ResourceLimit,
                "TIFF source byte allowance",
            ));
        }
        self.guard_asset_adoption(&path, Some(&identity))?;
        let service = self.editing.as_ref().ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Background image import is unavailable",
            )
        })?;
        let metadata = service.metadata_budget();
        let input = path.clone();
        let captured = identity.clone();
        let job = service.submit_tiff(move |cancel| {
            worker::inventory(captured, input, length, metadata, &|| cancel.check())
                .map(TiffCompletion::Inventory)
        })?;
        self.stage_candidate(
            Candidate::Tiff(Box::new(Source {
                stamp: self.editor.preview_stamp(),
                job: Some(job),
                frozen: None,
                selected: None,
                prepared: None,
                review: None,
                error: None,
            })),
            path.clone(),
            String::new(),
            true,
        )?;
        self.pending_import
            .as_mut()
            .unwrap()
            .auxiliary_sources
            .push(crate::destination::SourceGuard { path, identity });
        Ok(())
    }
    pub(super) fn tiff_import_dialog(&mut self, ctx: &egui::Context) {
        self.poll_tiff_import(ctx);
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        let Candidate::Tiff(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let name = pending
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let mut accept = false;
        let response = if source.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(20));
            let modal = egui::Modal::new(egui::Id::unique("tiff-page-preparing")).show(ctx, |ui| {
                ui.heading(t!(ctx, "Import image"));
                ui.label(name.as_ref());
                ui.spinner();
                ui.label(t!(ctx, "Preparing picture…"));
                ui.button(t!(ctx, "Cancel")).clicked()
            });
            if modal.inner || modal.should_close() {
                Response::Cancel
            } else {
                Response::Pending
            }
        } else if let (Some(prepared), Some(review)) = (&source.prepared, &mut source.review) {
            let modal = egui::Modal::new(egui::Id::unique("tiff-page-review")).show(ctx, |ui| {
                ui.heading(t!(ctx, "Import image"));
                ui.label(name.as_ref());
                let ready = review.show(ui, prepared);
                if let Some(error) = &source.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.horizontal(|ui| {
                    accept = ui
                        .add_enabled(ready, egui::Button::new(t!(ctx, "Open image")))
                        .clicked();
                    ui.button(t!(ctx, "Cancel")).clicked()
                })
                .inner
            });
            if modal.inner || modal.should_close() {
                Response::Cancel
            } else {
                Response::Pending
            }
        } else if let Some(frozen) = &source.frozen {
            page_import::dialog_profiled(
                ctx,
                page_import::Format::Tiff,
                &name,
                &frozen.choices,
                &mut source.selected,
                pages::PIXELS_MAX,
                source.error.as_deref(),
            )
        } else {
            Response::Cancel
        };
        self.pending_import = Some(pending);
        if ctx.current_pass_index() != 0 {
            return;
        }
        match response {
            Response::Cancel => self.resolve_import(ctx, false),
            Response::Import(index) => self.choose_tiff_page(ctx, index),
            Response::Pending if accept => self.accept_tiff_import(ctx),
            Response::Pending => {}
        }
    }
    pub(super) fn poll_tiff_import(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Tiff(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        if source.stamp != self.editor.preview_stamp() {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        if let Some(result) = source.job.as_ref().and_then(|job| job.poll()) {
            // Poll retires the executor reservation; dropping the handle here
            // prevents the chooser from retaining a 2GiB transient job charge.
            source.job = None;
            match result {
                Ok(TiffCompletion::Inventory(frozen)) => source.frozen = Some(frozen),
                Ok(TiffCompletion::Decoded(prepared)) => {
                    match review::Review::new(ctx, &prepared) {
                        Ok(review) => {
                            source.prepared = Some(prepared);
                            source.review = Some(review);
                        }
                        Err(error) => source.error = Some(error.to_string()),
                    }
                }
                Err(error) => {
                    source.error = Some(error.to_string());
                    if source.frozen.is_none() {
                        self.confirm = None;
                        self.editor.set_status(format!("Cannot open: {error}"));
                        return;
                    }
                }
            }
        }
        self.pending_import = Some(pending);
    }
    pub(super) fn choose_tiff_page(&mut self, ctx: &egui::Context, index: u16) {
        self.poll_tiff_import(ctx);
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        let result = (|| {
            if !pending.current(self) {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "TIFF import owner changed",
                ));
            }
            admit(&pending.path)?;
            let Candidate::Tiff(source) = &mut pending.candidate else {
                return Ok(());
            };
            if source.job.is_some() || source.prepared.is_some() {
                return Ok(());
            }
            let frozen = source
                .frozen
                .clone()
                .ok_or_else(|| Error::new(ErrorCode::InputSchema, "TIFF inventory is not ready"))?;
            if !frozen.choices.iter().any(|choice| {
                choice.page.index == index
                    && choice.supported
                    && u64::from(choice.page.width) * u64::from(choice.page.height)
                        <= pages::PIXELS_MAX
            }) {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "TIFF page is unavailable",
                ));
            }
            let service = self.editing.as_ref().ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Background image import is unavailable",
                )
            })?;
            let metadata = service.metadata_budget();
            let residency = service.import_residency();
            let input = pending.path.clone();
            source.job = Some(service.submit_tiff(move |cancel| {
                worker::decode(frozen, input, index, metadata, residency, &|| {
                    cancel.check()
                })
                .map(TiffCompletion::Decoded)
            })?);
            source.selected = Some(index);
            source.error = None;
            Ok(())
        })();
        if let Err(error) = result
            && let Candidate::Tiff(source) = &mut pending.candidate
        {
            source.error = Some(error.to_string());
        }
        self.pending_import = Some(pending);
    }
    pub(super) fn accept_tiff_import(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::Tiff(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        let result = (|| {
            if source.stamp != self.editor.preview_stamp()
                || !source
                    .review
                    .as_ref()
                    .is_some_and(|review| review.ready(ctx))
            {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "TIFF preview consent is not ready",
                ));
            }
            let prepared = source.prepared.as_ref().ok_or_else(|| {
                Error::new(ErrorCode::InputSchema, "TIFF selected page is not ready")
            })?;
            if source.selected != Some(prepared.assessment.page_index())
                || prepared.assessment.source_sha256() != prepared.frozen.inventory.source_sha256()
            {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "TIFF assessment changed",
                ));
            }
            admit(&pending.path)?;
            self.guard_asset_adoption(&pending.path, Some(&prepared.frozen.identity))?;
            prepared.frozen.verify(&pending.path, &|| Ok(()))?;
            Ok(())
        })();
        if let Err(error) = result {
            source.error = Some(error.to_string());
            self.pending_import = Some(pending);
            return;
        }
        let prepared = *source.prepared.take().unwrap();
        pending.digest = hex(&prepared.frozen.inventory.source_sha256());
        pending.captured_source = Some(CapturedSource {
            identity: prepared.frozen.identity.clone(),
            bytes: prepared.frozen.inventory.source_bytes(),
            sha256: prepared.frozen.inventory.source_sha256(),
        });
        pending.candidate = Candidate::Layered(paintcraft_formats::project_import::Converted {
            document: prepared.document,
            losses: prepared.losses,
        });
        self.pending_import = Some(pending);
        self.resolve_import(ctx, true);
    }
}

#[cfg(test)]
#[path = "tiff/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tiff/orientation_consumer_tests.rs"]
mod orientation_consumer_tests;
