//! Native Studio opt-in textures; legacy effective defaults keep the old route.
use super::*;
use crate::export_sources::Sources;
use export::metadata_policy::{Intent, Policy, Target};
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    dds,
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::{
    dds_export::{self as controls, Settings},
    i18n::translate,
    t,
};
#[path = "dds_export/view.rs"]
mod view;
#[path = "dds_export/worker.rs"]
pub(crate) mod worker;
#[derive(Debug)]
struct Capture {
    has_filters: bool,
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
    frozen: Option<export_options::Frozen>,
    omit_source_metadata: bool,
    metadata_policy: Policy,
    bake_filters: bool,
    filter_bake: export::filter_bake::Policy,
    settings: Settings,
    capture: Option<Capture>,
    pending: Option<controller::background::DdsExportPending>,
    prepared: Option<worker::Prepared>,
    texture: Option<egui::TextureHandle>,
    shown: Option<u64>,
    destination: Option<(PathBuf, Option<save::Expectation>)>,
    error: Option<String>,
}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DdsExportState")
            .field("settings", &self.settings)
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
pub(super) fn effective(mut settings: Settings, geometry: (u32, u32)) -> Result<Settings> {
    let count = settings
        .level_count(geometry.0, geometry.1)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::InputSchema,
                "DDS mip count is outside the image geometry",
            )
        })?;
    if count == 1 {
        settings.mips = controls::Mips::None;
        settings.filter = controls::Filter::Nearest;
    }
    if settings.encoding == controls::Encoding::Rgba8 {
        settings.quality = controls::Quality::Normal;
    }
    Ok(settings)
}
pub(super) fn advanced(settings: Settings, geometry: (u32, u32)) -> bool {
    effective(settings, geometry).is_ok_and(|s| s != Settings::default())
}
pub(super) fn admit(frontend: registry::Frontend, path: &Path) -> Result<()> {
    let mut request = Request::new(
        frontend,
        Operation::ExportFlat,
        Family::Dds,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|s| s.to_str());
    request.variant = Some(dds::NATIVE_TEXTURE_EXPORT_VARIANT);
    if request.extension.is_none() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "DDS output needs its selected extension",
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
    fn current(&self, app: &App) -> bool {
        self.tab == app.active_tab_id()
            && self.prepared.as_ref().is_none_or(|prepared| {
                prepared.artifact.encoded().plan().options() == worker::options(self.settings)
            })
            && app.experience() == Experience::Studio
            && self
                .frozen
                .as_ref()
                .map_or_else(|| !app.has_document_drafts(), |f| f.current(app))
            && !app.editor.has_floating_selection()
            && self.capture.as_ref().is_none_or(|c| {
                self.frozen.as_ref().map_or_else(
                    || Arc::ptr_eq(&c.doc, &app.editor.document_snapshot()),
                    |f| Arc::ptr_eq(&c.doc, &f.document),
                ) && c.stamp == app.editor.preview_stamp()
                    && c.owner == app.editor.native_import_owner()
                    && c.dirty == app.editor.is_dirty()
                    && c.bound == app.bound
                    && c.path.as_deref() == app.editor.path()
                    && c.text == app.path_text
                    && app.editor.can_capture_export()
            })
    }
    fn has_source_metadata(&self) -> bool {
        self.capture
            .as_ref()
            .is_some_and(|c| c.doc.source_metadata.is_some())
            || self
                .frozen
                .as_ref()
                .is_some_and(|f| f.document.source_metadata.is_some())
    }
    fn has_filters(&self) -> bool {
        self.capture.as_ref().is_some_and(|c| c.has_filters)
            || self.frozen.as_ref().is_some_and(|f| f.has_filters)
    }
    fn metadata_ready(&self) -> bool {
        (!self.has_source_metadata() || self.omit_source_metadata)
            && (!self.has_filters() || self.bake_filters)
    }
    fn retire(&mut self) {
        self.metadata_policy = Policy::Refuse;
        self.filter_bake = Default::default();
        self.pending = None;
        self.prepared = None;
        self.texture = None;
        self.shown = None;
        self.destination = None;
    }
}
impl App {
    #[cfg(test)]
    pub(super) fn begin_dds_export(&mut self, settings: Settings) {
        self.begin_dds_export_source(settings, None, Intent::Refuse);
    }
    pub(super) fn begin_dds_export_frozen(
        &mut self,
        settings: Settings,
        frozen: export_options::Frozen,
        intent: Intent,
    ) {
        if self.editor.failed_text_retained() && !self.editor.can_capture_export() {
            return;
        }

        self.begin_dds_export_source(settings, Some(frozen), intent);
    }
    fn begin_dds_export_source(
        &mut self,
        settings: Settings,
        frozen: Option<export_options::Frozen>,
        intent: Intent,
    ) {
        let result = (|| {
            admit(self.format_frontend(), Path::new("picture.dds"))?;
            if self.document_count() == 0
                || frozen
                    .as_ref()
                    .map_or_else(|| self.has_document_drafts(), |f| !f.current(self))
                || self.editor.has_floating_selection()
                || self.confirm.is_some()
                || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            {
                return Err(stale());
            }
            self.dds_export = Some(State {
                tab: self.active_tab_id(),
                frozen,
                omit_source_metadata: intent == Intent::Omit,
                metadata_policy: Policy::Refuse,
                bake_filters: false,
                filter_bake: Default::default(),
                settings: effective(settings, self.editor.document().canvas)?,
                capture: None,
                pending: None,
                prepared: None,
                texture: None,
                shown: None,
                destination: None,
                error: None,
            });
            self.confirm = Some(Confirm::DdsExport);
            if self
                .dds_export
                .as_ref()
                .is_some_and(|state| state.frozen.is_some())
            {
                self.capture_dds_export();
            } else {
                self.defer_file(
                    editing::FileRequest::ExportDds,
                    token(&self.context, code::EXPORT),
                );
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.dds_error(error);
        }
    }
    pub(super) fn dds_waiting_for_edits(&self) -> bool {
        self.dds_export
            .as_ref()
            .is_some_and(|s| s.tab == self.active_tab_id() && s.capture.is_none())
    }
    pub(super) fn capture_dds_export(&mut self) {
        let Some(mut state) = self.dds_export.take() else {
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
            let doc = state
                .frozen
                .as_ref()
                .map(|f| f.document.clone())
                .unwrap_or_else(|| self.editor.document_snapshot());
            state.capture = Some(Capture {
                has_filters: doc.layers.values().any(|l| !l.filters.is_empty()),
                doc,
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
            Ok(()) => self.dds_export = Some(state),
            Err(error) => self.dds_error(error),
        }
    }
    pub(super) fn poll_dds_export(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.dds_export.take() else {
            return;
        };
        if (!self.dds_export_picker && self.confirm != Some(Confirm::DdsExport))
            || !state.current(self)
        {
            if self.confirm == Some(Confirm::DdsExport) {
                self.confirm = None;
            }
            return;
        }
        if let Some(result) = state.pending.as_ref().and_then(|p| p.poll()) {
            state.pending = None;
            match result {
                Ok(prepared) => state.prepared = Some(prepared),
                Err(error) => {
                    state.error = Some(self.dds_preparation_error(&error));
                }
            }
        }
        if state.pending.is_none()
            && state.prepared.is_none()
            && state.error.is_none()
            && state.metadata_ready()
            && let Some(capture) = &state.capture
        {
            let result = (|| {
                let service = self.editing.as_ref().ok_or_else(stale)?;
                service.reap_abandoned();
                let quote = worker::quote(&capture.doc, state.settings)?;
                let doc = capture.doc.clone();
                let policy = Policy::capture(
                    &doc,
                    Target::new(Family::Dds, Operation::ExportFlat),
                    if state.omit_source_metadata {
                        Intent::Omit
                    } else {
                        Intent::Refuse
                    },
                )?;
                state.filter_bake = export::filter_bake::Policy::capture(
                    &doc,
                    Target::new(Family::Dds, Operation::ExportFlat),
                    if state.bake_filters {
                        export::filter_bake::Intent::Bake
                    } else {
                        export::filter_bake::Intent::Refuse
                    },
                    &|| Ok(()),
                )?;
                let policies = export::CopyPolicies {
                    source_metadata: policy,
                    filter_bake: state.filter_bake,
                };
                state.metadata_policy = policy;
                let settings = state.settings;
                service.try_submit_dds_export(quote, move |cancel| {
                    worker::prepare_with_policies(doc, settings, quote, cancel, policies)
                })
            })();
            match result {
                Ok(Some(pending)) => state.pending = Some(pending),
                Ok(None) => {
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
                Err(error) => state.error = Some(self.dds_preparation_error(&error)),
            }
        }
        if let Some(prepared) = &mut state.prepared
            && let Some(image) = prepared.preview.take()
        {
            state.texture =
                Some(ctx.load_texture("dds-export-preview", image, egui::TextureOptions::NEAREST));
            state.shown = None;
        }
        if state.pending.is_some() || state.capture.is_none() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        self.dds_export = Some(state);
    }
    fn choose_dds_destination(&mut self, ctx: &egui::Context) {
        let suggested = Path::new(&self.path_text).with_extension("dds");
        if let Some(dialogs) = &mut self.dialogs {
            if dialogs.pending() {
                self.dds_error(stale());
                return;
            }
            self.confirm = None;
            self.dds_export_picker = true;
            let wake = ctx.clone();
            dialogs.request_with_filters(
                Kind::Export,
                Some(&suggested),
                &[paintcraft_platform::dialogs::Filter {
                    name: t!(ctx, "DDS texture"),
                    extensions: vec!["dds".into()],
                }],
                |text| t!(ctx, text),
                move || wake.request_repaint(),
            );
        } else {
            self.complete_dds_picker(Some(suggested));
        }
    }
    pub(super) fn complete_dds_picker(&mut self, path: Option<PathBuf>) {
        let Some(mut state) = self.dds_export.take() else {
            return;
        };
        self.confirm = None;
        let Some(path) = path else {
            return;
        };
        let result = (|| {
            if !state.current(self) {
                return Err(stale());
            }
            admit(self.format_frontend(), &path)?;
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
                        Error::new(ErrorCode::StorageFailed, "Cannot resolve DDS destination")
                    })?;
                    let acquired = crate::format_admission::acquire_resource_source(
                        &resolved,
                        dds::BYTES_MAX as u64,
                    )?;
                    Some(save::Expectation::acquired(&resolved, &acquired.bytes))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => {
                    return Err(Error::new(
                        ErrorCode::StorageFailed,
                        "Cannot inspect DDS destination",
                    ));
                }
            };
            Ok(expected)
        })();
        match result {
            Ok(expected) => {
                state.destination = Some((path, expected));
                self.dds_export = Some(state);
                self.confirm = Some(Confirm::DdsExport);
            }
            Err(error) => self.dds_error(error),
        }
    }
    fn finish_dds_export(&mut self) {
        let Some(state) = self.dds_export.take() else {
            return;
        };
        self.confirm = None;
        let result = (|| {
            let capture = state.capture.as_ref().ok_or_else(stale)?;
            let prepared = state.prepared.as_ref().ok_or_else(stale)?;
            let (path, expected) = state.destination.as_ref().ok_or_else(stale)?;
            let check = || {
                if matches!(state.filter_bake, export::filter_bake::Policy::Bake(_))
                    != state.bake_filters
                    || !state.metadata_ready()
                {
                    return Err(stale());
                }
                state.filter_bake.validate(
                    &capture.doc,
                    Target::new(Family::Dds, Operation::ExportFlat),
                    &|| Ok(()),
                )?;
                state.metadata_policy.validate(
                    &capture.doc,
                    Target::new(Family::Dds, Operation::ExportFlat),
                )?;
                if !state.current(self) || prepared.artifact.source_state() != capture.doc.state_id
                {
                    return Err(stale());
                }
                admit(self.format_frontend(), path)?;
                self.guard_export_destination(path)?;
                capture.sources.check_destination(path)
            };
            check()?;
            Ok(save::save_with_expectation_and_preparation(
                path,
                expected.is_some(),
                dds::BYTES_MAX as u64,
                expected.as_ref(),
                |sink| {
                    std::io::Write::write_all(sink, prepared.artifact.encoded().bytes()).map_err(
                        |_| Error::new(ErrorCode::StorageFailed, "Cannot stage DDS output"),
                    )
                },
                |_| check(),
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
            Err(error) => self.dds_error(error),
        }
    }
    fn dds_preparation_error(&mut self, error: &Error) -> String {
        if error.code == ErrorCode::ResourceLimit {
            self.editor.set_status(format!("Cannot export: {error}"));
            // Admission may fail before the first UI pass installs its locale.
            translate(
                self.settings.language,
                "Not enough memory to export with these settings. Use a smaller image or fewer layers.",
            )
        } else if error.message.contains("nonopaque") {
            translate(
                self.settings.language,
                "BC1 requires a picture without transparency. Choose RGBA8, BC2 or BC3.",
            )
        } else {
            error.to_string()
        }
    }
    fn dds_error(&mut self, error: Error) {
        self.dds_export = None;
        if self.confirm == Some(Confirm::DdsExport) {
            self.confirm = None;
        }
        let message = if error.code == ErrorCode::ConflictStaleBase {
            translate(
                self.settings.language,
                "The document changed; choose the image again.",
            )
        } else if error.message.contains("nonopaque") {
            translate(
                self.settings.language,
                "BC1 requires a picture without transparency. Choose RGBA8, BC2 or BC3.",
            )
        } else {
            error.to_string()
        };
        self.editor.set_status(format!("Cannot export: {message}"));
    }
}
#[cfg(test)]
#[path = "dds_export/tests.rs"]
mod tests;
