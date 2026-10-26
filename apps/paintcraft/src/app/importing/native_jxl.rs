//! Isolated JXL conversion retains App ownership and existing editing admission.
use super::*;
use paintcraft_formats::{
    codec_worker::{
        self as wire,
        native::{Job, PreparedImport},
    },
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_platform::source::SourceIdentity;
use paintcraft_runtime::process;
use std::time::Duration;

pub(super) struct Source {
    owner: wire::Owner,
    stamp: controller::PreviewStamp,
    job: Option<controller::background::ImportPending>,
    prepared: Option<PreparedImport>,
}
fn owner(app: &App) -> wire::Owner {
    let mut owner = app.editor.native_import_owner();
    if app.experience() != Experience::Studio {
        owner.frontend = 0;
    }
    owner
}
impl Source {
    fn current(&self, app: &App) -> bool {
        owner(app) == self.owner && app.editor.preview_stamp() == self.stamp
    }
}
type Launch =
    Box<dyn FnOnce(wire::native::Launch<'_>, &dyn Fn() -> Result<()>) -> Result<Vec<u8>> + Send>;
fn launch(launch: wire::native::Launch<'_>, check: &dyn Fn() -> Result<()>) -> Result<Vec<u8>> {
    process::run_parent_lease(
        launch.executable,
        launch.arguments,
        launch.request.segments(),
        process::Limits {
            input_bytes: wire::INPUT_BYTES_MAX + wire::REQUEST_HEADER,
            output_bytes: wire::OUTPUT_BYTES_MAX,
            deadline: launch.remaining,
            resource_exit_code: Some(wire::RESOURCE_EXIT),
        },
        check,
    )
    .map(|output| output.bytes)
}
impl App {
    pub(crate) fn stage_native_jxl(&mut self, path: PathBuf) -> Result<()> {
        self.stage_native_jxl_with_launch(path, Box::new(launch))
    }
    fn stage_native_jxl_with_launch(&mut self, path: PathBuf, launch: Launch) -> Result<()> {
        let mut request = Request::new(
            self.format_frontend(),
            Operation::ImportConvert,
            Family::Jxl,
            Platform::Native,
        );
        request.extension = path.extension().and_then(|value| value.to_str());
        request.variant = Some(wire::NATIVE_JXL_IMPORT_VARIANT);
        registry::admission(request).require_allowed(request)?;
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
                "Native codec input must be a regular file",
            )
        })?;
        let length = identity
            .file()
            .metadata()
            .map_err(|e| Error::new(ErrorCode::StorageFailed, e.to_string()))?
            .len();
        wire::reservation_bytes_for_input(length)?;
        self.guard_asset_adoption(&path, Some(&identity))?;
        let service = self.editing.as_ref().ok_or_else(|| {
            Error::new(
                ErrorCode::CapabilityUnsupported,
                "Background image import is unavailable",
            )
        })?;
        let residency = service.import_residency();
        let captured = owner(self);
        let input = path.clone();
        let job_identity = identity.clone();
        let job = service.submit_import(move |cancel| {
            let lease = cancel.resource_lease().ok_or_else(|| {
                Error::new(ErrorCode::ResourceLimit, "Native codec admission missing")
            })?;
            let check = || cancel.check();
            Job {
                input,
                identity: job_identity,
                length,
                codec: wire::Codec::JxlRgba8,
                owner: captured,
                pixels: wire::PIXELS_MAX,
                deadline: Duration::from_secs(10),
                application_version: env!("CARGO_PKG_VERSION").into(),
                expected_sha256: None,
            }
            .prepare_jxl(residency, lease, &check, &|| Ok(captured), launch)
        })?;
        self.stage_candidate(
            Candidate::NativeJxl(Box::new(Source {
                owner: captured,
                stamp: self.editor.preview_stamp(),
                job: Some(job),
                prepared: None,
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
    pub(super) fn native_jxl_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.pending_import.take() else {
            return;
        };
        if !pending.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let Candidate::NativeJxl(source) = &mut pending.candidate else {
            self.pending_import = Some(pending);
            return;
        };
        if !source.current(self) {
            self.confirm = None;
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        if let Some(result) = source.job.as_ref().and_then(|job| job.poll()) {
            source.job = None;
            match result {
                Ok(prepared) => source.prepared = Some(prepared),
                Err(error) => {
                    self.confirm = None;
                    self.editor.set_status(format!("Cannot open: {error}"));
                    return;
                }
            }
        }
        let response = egui::Modal::new(egui::Id::unique("native-jxl-import")).show(ctx, |ui| {
            ui.heading(t!(ctx, "Import image"));
            ui.add(
                egui::Label::new(
                    pending
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                )
                .truncate(),
            );
            if source.prepared.is_none() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t!(ctx, "Preparing picture…"));
                });
                ctx.request_repaint_after(Duration::from_millis(16));
            }
            ui.button(t!(ctx, "Cancel")).clicked()
        });
        if ctx.current_pass_index() != 0 {
            ctx.request_repaint();
            self.pending_import = Some(pending);
            return;
        }
        if response.inner || response.should_close() {
            self.confirm = None;
            self.editor.set_status(t!(ctx, "Import cancelled."));
            return;
        }
        if source.prepared.is_some() {
            let result = (|| {
                if !source.current(self) {
                    return Err(Error::new(
                        ErrorCode::ConflictStaleBase,
                        "Codec import owner changed before adoption",
                    ));
                }
                let prepared = source.prepared.take().ok_or_else(|| {
                    Error::new(ErrorCode::ConflictStaleBase, "Codec import is not ready")
                })?;
                let adopted = prepared.adopt(&|| Ok(()), &|| Ok(owner(self)))?;
                self.guard_asset_adoption(&adopted.input, Some(&adopted.identity))?;
                let mut editor = Editor::with_document(adopted.document, None);
                editor.invalidate_saved_state();
                self.install(editor)?;
                self.bound = None;
                self.imported_source = Some(adopted.input.clone());
                self.auxiliary_sources = pending.auxiliary_sources;
                self.path_text = adopted
                    .input
                    .with_extension("pcnd")
                    .to_string_lossy()
                    .into_owned();
                self.editor
                    .set_status(format!("Imported {}", adopted.input.display()));
                Ok::<_, Error>(())
            })();
            self.confirm = None;
            if let Err(error) = result {
                self.editor.set_status(format!("Cannot open: {error}"));
            }
            return;
        }
        self.pending_import = Some(pending);
    }
}
#[cfg(test)]
#[path = "native_jxl/installed_tests.rs"]
mod installed_tests;
#[cfg(test)]
#[path = "native_jxl/tests.rs"]
mod tests;
