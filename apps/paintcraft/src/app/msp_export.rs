//! Native Studio MSP copies, with retained exact preparation and source guards.
use super::*;
use crate::export_sources::Sources;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    msp,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{
    i18n::translate,
    msp_export::{self as controls, Version},
    t,
};
#[path = "msp_export/view.rs"]
mod view;
#[path = "msp_export/worker.rs"]
pub(crate) mod worker;
#[derive(Debug)]
struct Capture {
    doc: Arc<paintcraft_document::Document>,
    stamp: controller::PreviewStamp,
    owner: paintcraft_formats::codec_worker::Owner,
    dirty: bool,
    bound: Option<PathBuf>,
    path: Option<PathBuf>,
    text: String,
    sources: Sources,
}
pub(super) struct State {
    tab: Uuid,
    version: Version,
    capture: Option<Capture>,
    pending: Option<controller::background::MspExportPending>,
    prepared: Option<worker::Prepared>,
    texture: Option<egui::TextureHandle>,
    shown: Option<u64>,
    destination: Option<(PathBuf, Option<save::Expectation>)>,
    error: Option<String>,
    losses_accepted: bool,
}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MspExportState")
            .field("version", &self.version)
            .field("capture", &self.capture)
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}
fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "The document changed; choose the image again.",
    )
}
pub(super) fn admit(frontend: registry::Frontend, path: &Path, version: Version) -> Result<()> {
    let mut request = Request::new(
        frontend,
        Operation::ExportFlat,
        Family::Msp,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|s| s.to_str());
    if version == Version::RowRleV2 {
        request.variant = Some(msp::NATIVE_V2_EXPORT_VARIANT);
    }
    if !request
        .extension
        .is_some_and(|extension| extension.eq_ignore_ascii_case("msp"))
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "MSP output needs its selected extension",
        ));
    }
    match registry::admission(request) {
        registry::Decision::Allowed
        | registry::Decision::AllowedWithPreflight(registry::Reason::ConvertRequiresConsent) => {
            Ok(())
        }
        decision => decision.require_allowed(request),
    }
}
impl State {
    #[cfg(test)]
    pub(super) fn preparation_finished(&self) -> bool {
        self.texture.is_some() || self.error.is_some()
    }
    fn current(&self, app: &App) -> bool {
        self.tab == app.active_tab_id()
            && self.prepared.as_ref().is_none_or(|prepared| {
                prepared.artifact.plan().layout().version() == worker::version(self.version)
            })
            && app.experience() == Experience::Studio
            && !app.has_document_drafts()
            && !app.editor.has_floating_selection()
            && self.capture.as_ref().is_none_or(|c| {
                Arc::ptr_eq(&c.doc, &app.editor.document_snapshot())
                    && c.stamp == app.editor.preview_stamp()
                    && c.owner == app.editor.native_import_owner()
                    && c.dirty == app.editor.is_dirty()
                    && c.bound == app.bound
                    && c.path.as_deref() == app.editor.path()
                    && c.text == app.path_text
                    && !app.editor.has_pending_edits()
            })
    }
    fn retire(&mut self) {
        self.pending = None;
        self.prepared = None;
        self.texture = None;
        self.shown = None;
        self.destination = None;
        self.losses_accepted = false;
    }
}
impl App {
    pub(super) fn begin_msp_export(&mut self, version: Version) {
        let result = (|| {
            admit(self.format_frontend(), Path::new("picture.msp"), version)?;
            if self.document_count() == 0
                || self.has_document_drafts()
                || self.editor.has_floating_selection()
                || self.confirm.is_some()
                || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            {
                return Err(stale());
            }
            self.msp_export = Some(State {
                tab: self.active_tab_id(),
                version,
                capture: None,
                pending: None,
                prepared: None,
                texture: None,
                shown: None,
                destination: None,
                error: None,
                losses_accepted: false,
            });
            self.defer_file(
                editing::FileRequest::ExportMsp,
                token(&self.context, code::EXPORT),
            );
            self.confirm = Some(Confirm::MspExport);
            Ok(())
        })();
        if let Err(error) = result {
            self.msp_error(error);
        }
    }
    pub(super) fn msp_waiting_for_edits(&self) -> bool {
        self.msp_export
            .as_ref()
            .is_some_and(|s| s.tab == self.active_tab_id() && s.capture.is_none())
    }
    pub(super) fn capture_msp_export(&mut self) {
        let Some(mut state) = self.msp_export.take() else {
            return;
        };
        let result = (|| {
            if !state.current(self) {
                return Err(stale());
            }
            let service = self.editing.as_ref().ok_or_else(|| {
                Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Background image export is unavailable",
                )
            })?;
            state.capture = Some(Capture {
                doc: self.editor.document_snapshot(),
                stamp: self.editor.preview_stamp(),
                owner: self.editor.native_import_owner(),
                dirty: self.editor.is_dirty(),
                bound: self.bound.clone(),
                path: self.editor.path().map(Path::to_path_buf),
                text: self.path_text.clone(),
                sources: Sources::capture(self, &service.metadata_budget())?,
            });
            Ok(())
        })();
        match result {
            Ok(()) => self.msp_export = Some(state),
            Err(error) => self.msp_error(error),
        }
    }
    pub(super) fn poll_msp_export(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.msp_export.take() else {
            return;
        };
        if (!self.msp_export_picker && self.confirm != Some(Confirm::MspExport))
            || !state.current(self)
        {
            if self.confirm == Some(Confirm::MspExport) {
                self.confirm = None;
            }
            return;
        }
        if let Some(result) = state.pending.as_ref().and_then(|p| p.poll()) {
            state.pending = None;
            match result {
                Ok(prepared) => state.prepared = Some(prepared),
                Err(error) => {
                    state.error = Some(self.msp_preparation_error(&error));
                }
            }
        }
        if state.pending.is_none()
            && state.prepared.is_none()
            && state.error.is_none()
            && let Some(capture) = &state.capture
        {
            let result = (|| {
                admit(
                    self.format_frontend(),
                    Path::new("picture.msp"),
                    state.version,
                )?;
                let service = self.editing.as_ref().ok_or_else(stale)?;
                service.reap_abandoned();
                let quote = worker::quote(&capture.doc, state.version)?;
                let doc = capture.doc.clone();
                let version = state.version;
                service.try_submit_msp_export(quote, move |cancel, capacity| {
                    worker::prepare(doc, version, quote, capacity, cancel)
                })
            })();
            match result {
                Ok(Some(pending)) => state.pending = Some(pending),
                Ok(None) => {
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
                Err(error) => state.error = Some(self.msp_preparation_error(&error)),
            }
        }
        if let Some(prepared) = &mut state.prepared
            && let Some(image) = prepared.preview.take()
        {
            state.texture =
                Some(ctx.load_texture("msp-export-preview", image, egui::TextureOptions::NEAREST));
            state.shown = None;
        }
        if state.pending.is_some() || state.capture.is_none() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        self.msp_export = Some(state);
    }
    fn choose_msp_destination(&mut self, ctx: &egui::Context) {
        let suggested = Path::new(&self.path_text).with_extension("msp");
        if let Some(dialogs) = &mut self.dialogs {
            if dialogs.pending() {
                self.msp_error(stale());
                return;
            }
            self.confirm = None;
            self.msp_export_picker = true;
            let wake = ctx.clone();
            dialogs.request_with_filters(
                Kind::Export,
                Some(&suggested),
                &[paintcraft_platform::dialogs::Filter {
                    name: t!(ctx, "MSP picture"),
                    extensions: vec!["msp".into()],
                }],
                |text| t!(ctx, text),
                move || wake.request_repaint(),
            );
        } else {
            self.complete_msp_picker(Some(suggested));
        }
    }
    pub(super) fn complete_msp_picker(&mut self, path: Option<PathBuf>) {
        let Some(mut state) = self.msp_export.take() else {
            return;
        };
        // A delayed picker owns this export, never a newer application's modal.
        if self.confirm.is_some() && self.confirm != Some(Confirm::MspExport) {
            return;
        }
        self.confirm = None;
        let Some(path) = path else {
            return;
        };
        let result = (|| {
            if !state.current(self) {
                return Err(stale());
            }
            admit(self.format_frontend(), &path, state.version)?;
            self.guard_export_destination(&path)?;
            state
                .capture
                .as_ref()
                .ok_or_else(stale)?
                .sources
                .check_destination(&path)?;
            let expected = match std::fs::symlink_metadata(&path) {
                Ok(_) => {
                    let resolved = path.canonicalize().map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Cannot resolve MSP destination")
                    })?;
                    let acquired = crate::format_admission::acquire_resource_source(
                        &resolved,
                        msp::BYTES_MAX as u64,
                    )?;
                    Some(save::Expectation::acquired(&resolved, &acquired.bytes))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => {
                    return Err(Error::new(
                        ErrorCode::StorageFailed,
                        "Cannot inspect MSP destination",
                    ));
                }
            };
            Ok(expected)
        })();
        match result {
            Ok(expected) => {
                state.destination = Some((path, expected));
                self.msp_export = Some(state);
                self.confirm = Some(Confirm::MspExport);
            }
            Err(error) => self.msp_error(error),
        }
    }
    fn finish_msp_export(&mut self) {
        let Some(state) = self.msp_export.take() else {
            return;
        };
        if self.confirm != Some(Confirm::MspExport) {
            return;
        }
        self.confirm = None;
        let result = (|| {
            let capture = state.capture.as_ref().ok_or_else(stale)?;
            let prepared = state.prepared.as_ref().ok_or_else(stale)?;
            let (path, expected) = state.destination.as_ref().ok_or_else(stale)?;
            if !prepared.artifact.losses().is_empty() && !state.losses_accepted {
                return Err(Error::new(
                    ErrorCode::InputSchema,
                    "MSP export changes need approval",
                ));
            }
            let check = || {
                if !state.current(self) || prepared.artifact.source_state() != capture.doc.state_id
                {
                    return Err(stale());
                }
                admit(self.format_frontend(), path, state.version)?;
                self.guard_export_destination(path)?;
                capture.sources.check_destination(path)
            };
            check()?;
            Ok(save::save_with_expectation_and_preparation(
                path,
                expected.is_some(),
                msp::BYTES_MAX as u64,
                expected.as_ref(),
                |sink| {
                    std::io::Write::write_all(sink, prepared.artifact.bytes()).map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Cannot stage MSP output")
                    })
                },
                |_| {
                    #[cfg(test)]
                    BEFORE_PUBLICATION.with(|hook| {
                        if let Some(hook) = hook.borrow_mut().take() {
                            hook();
                        }
                    });
                    check()
                },
            ))
        })();
        match result {
            Ok(outcome) => self.editor.set_status(if outcome.is_success() {
                format!(
                    "Exported {}",
                    state.destination.as_ref().unwrap().0.display()
                )
            } else {
                format!("Cannot export: {}", outcome.reasons.join("; "))
            }),
            Err(error) => self.msp_error(error),
        }
    }
    fn msp_preparation_error(&mut self, error: &Error) -> String {
        self.editor.set_status(format!("Cannot export: {error}"));
        if error.message == "MSP requires explicit opaque black/white conversion before export" {
            translate(
                self.settings.language,
                "MSP requires opaque black or white pixels. No automatic conversion is applied.",
            )
        } else if error.code == ErrorCode::ResourceLimit {
            translate(
                self.settings.language,
                "Not enough memory to export with these settings. Use a smaller image or fewer layers.",
            )
        } else {
            error.to_string()
        }
    }
    fn msp_error(&mut self, error: Error) {
        self.msp_export = None;
        if self.confirm == Some(Confirm::MspExport) {
            self.confirm = None;
        }
        self.editor.set_status(format!("Cannot export: {error}"));
    }
}
// Deterministic namespace mutation at the real post-stage checkpoint, tests only.
#[cfg(test)]
thread_local! {
    static BEFORE_PUBLICATION: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}
#[cfg(test)]
#[path = "msp_export/tests.rs"]
mod tests;
