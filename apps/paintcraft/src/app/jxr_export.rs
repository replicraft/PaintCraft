//! Native Studio fresh JPEG XR copies use the existing admitted worker and publisher.
use super::*;
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_formats::{
    codec_worker::{self as wire, jxr_encode as encode},
    registry::{self, Family, Operation, Platform, Request},
};
use paintcraft_ui::t;
#[path = "jxr_export/worker.rs"]
mod worker;
use crate::export_sources::Sources;
use export::metadata_policy::{Intent, Policy, Target};

type Prepared = controller::background::PreparedJxrExport;
#[derive(Debug)]
struct Capture {
    source: Arc<paintcraft_document::Document>,
    stamp: controller::PreviewStamp,
    owner: wire::Owner,
    dirty: bool,
    bound: Option<PathBuf>,
    editor_path: Option<PathBuf>,
    path_text: String,
    sources: Sources,
}
pub(super) struct State {
    tab: Uuid,
    frozen: Option<export_options::Frozen>,
    omit_source_metadata: bool,
    metadata_policy: Policy,
    capture: Option<Capture>,
    pending: Option<controller::background::JxrExportPending>,
    prepared: Option<Prepared>,
    texture: Option<egui::TextureHandle>,
    shown: Option<u64>,
    reviewed: bool,
    destination: Option<(PathBuf, Option<save::Expectation>)>,
}
impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JxrExportState")
            .field("tab", &self.tab)
            .field("capture", &self.capture)
            .field("pending", &self.pending)
            .finish_non_exhaustive()
    }
}
impl State {
    fn current(&self, app: &App) -> bool {
        self.tab == app.active_tab_id()
            && app.experience() == Experience::Studio
            && self
                .frozen
                .as_ref()
                .map_or_else(|| !app.has_document_drafts(), |f| f.current(app))
            && !app.editor.has_floating_selection()
            && self.capture.as_ref().is_none_or(|capture| {
                self.frozen.as_ref().map_or_else(
                    || Arc::ptr_eq(&capture.source, &app.editor.document_snapshot()),
                    |f| Arc::ptr_eq(&capture.source, &f.document),
                ) && capture.stamp == app.editor.preview_stamp()
                    && capture.owner == app.editor.native_import_owner()
                    && capture.dirty == app.editor.is_dirty()
                    && capture.bound == app.bound
                    && capture.editor_path.as_deref() == app.editor.path()
                    && capture.path_text == app.path_text
                    && app.editor.can_capture_export()
            })
    }
    fn has_source_metadata(&self) -> bool {
        self.capture
            .as_ref()
            .is_some_and(|c| c.source.source_metadata.is_some())
            || self
                .frozen
                .as_ref()
                .is_some_and(|f| f.document.source_metadata.is_some())
    }
    fn metadata_ready(&self) -> bool {
        !self.has_source_metadata() || self.omit_source_metadata
    }
    fn retire_preparation(&mut self) {
        self.pending = None;
        self.prepared = None;
        self.texture = None;
        self.shown = None;
        self.reviewed = false;
        self.destination = None;
        self.metadata_policy = Policy::Refuse;
    }
}
pub(super) fn admit(frontend: registry::Frontend, path: &Path) -> Result<()> {
    let mut request = Request::new(
        frontend,
        Operation::ExportFlat,
        Family::Jxr,
        Platform::Native,
    );
    request.extension = path.extension().and_then(|extension| extension.to_str());
    request.variant = Some(encode::qualified::EXPORT_VARIANT);
    if request.extension.is_none() {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "JPEG XR output requires its selected extension",
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
impl App {
    pub(super) fn begin_jxr_export_frozen(
        &mut self,
        frozen: export_options::Frozen,
        intent: Intent,
    ) {
        self.begin_jxr_export_source(Some(frozen), intent);
    }
    fn begin_jxr_export_source(&mut self, frozen: Option<export_options::Frozen>, intent: Intent) {
        if self.editor.failed_text_retained() && !self.editor.can_capture_export() {
            return;
        }

        if self.jxr_export.is_some()
            || self.confirm.is_some()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            self.editor.set_status(t!(
                &self.context,
                "Finish the current editing operation before exporting a picture."
            ));
            return;
        }
        let result = (|| {
            admit(self.format_frontend(), Path::new("picture.jxr"))?;
            if self.document_count() == 0
                || frozen
                    .as_ref()
                    .map_or_else(|| self.has_document_drafts(), |f| !f.current(self))
                || self.editor.has_floating_selection()
                || self.confirm.is_some()
                || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
            {
                return Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "Finish the current editing operation before exporting a picture",
                ));
            }
            if self.editing.is_none() {
                return Err(Error::new(
                    ErrorCode::CapabilityUnsupported,
                    "Background image export is unavailable",
                ));
            }
            self.jxr_export = Some(State {
                tab: self.active_tab_id(),
                frozen,
                omit_source_metadata: intent == Intent::Omit,
                metadata_policy: Policy::Refuse,
                capture: None,
                pending: None,
                prepared: None,
                texture: None,
                shown: None,
                reviewed: false,
                destination: None,
            });
            self.confirm = Some(Confirm::JxrExport);
            if self
                .jxr_export
                .as_ref()
                .is_some_and(|state| state.frozen.is_some())
            {
                self.capture_jxr_export();
            } else {
                self.defer_file(
                    editing::FileRequest::ExportJxr,
                    token(&self.context, code::EXPORT),
                );
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.export_jxr_error(error);
        }
    }
    pub(super) fn jxr_waiting_for_edits(&self) -> bool {
        self.jxr_export
            .as_ref()
            .is_some_and(|state| state.tab == self.active_tab_id() && state.capture.is_none())
    }
    pub(super) fn capture_jxr_export(&mut self) {
        let Some(mut state) = self.jxr_export.take() else {
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
            let source = state
                .frozen
                .as_ref()
                .map(|f| f.document.clone())
                .unwrap_or_else(|| self.editor.document_snapshot());
            let owner = self.editor.native_import_owner();
            let sources = Sources::capture(self, &service.metadata_budget())?;
            state.capture = Some(Capture {
                source: source.clone(),
                stamp: self.editor.preview_stamp(),
                owner,
                dirty: self.editor.is_dirty(),
                bound: self.bound.clone(),
                editor_path: self.editor.path().map(Path::to_path_buf),
                path_text: self.path_text.clone(),
                sources,
            });
            self.prepare_jxr_state(&mut state)?;
            Ok(())
        })();
        match result {
            Ok(()) => self.jxr_export = Some(state),
            Err(error) => self.export_jxr_error(error),
        }
    }
    fn prepare_jxr_state(&self, state: &mut State) -> Result<()> {
        if !state.metadata_ready() || state.pending.is_some() || state.prepared.is_some() {
            return Ok(());
        }
        let Some(capture) = state.capture.as_ref() else {
            return Ok(());
        };
        let source = capture.source.clone();
        let layout =
            encode::Layout::new(source.canvas.0, source.canvas.1, encode::ENCODED_BYTES_MAX)?;
        let quote = worker::quote(layout)?;
        let owner = capture.owner;
        let policy = Policy::capture(
            &source,
            Target::new(Family::Jxr, Operation::ExportFlat),
            if state.omit_source_metadata {
                Intent::Omit
            } else {
                Intent::Refuse
            },
        )?;
        let service = self.editing.as_ref().ok_or_else(stale)?;
        service.reap_abandoned();
        state.pending = service.try_submit_jxr_export(quote, move |cancel| {
            worker::prepare(source, owner, cancel, policy)
        })?;
        state.metadata_policy = policy;
        Ok(())
    }
    pub(super) fn poll_jxr_export(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.jxr_export.take() else {
            return;
        };
        if !self.jxr_export_picker && self.confirm != Some(Confirm::JxrExport) {
            return; // Retire this job without replacing a newly opened foreign modal.
        }
        if !state.current(self) {
            self.export_jxr_error(stale());
            return;
        }
        if let Err(error) = self.prepare_jxr_state(&mut state) {
            self.export_jxr_error(error);
            return;
        }
        if let Some(result) = state.pending.as_ref().and_then(|pending| pending.poll()) {
            state.pending = None;
            match result {
                Ok(prepared) => state.prepared = Some(prepared),
                Err(error) => {
                    self.export_jxr_error(error);
                    return;
                }
            }
        }
        if let Some(prepared) = &mut state.prepared
            && let Some(image) = prepared.preview.take()
        {
            state.texture =
                Some(ctx.load_texture("jxr-export-preview", image, egui::TextureOptions::NEAREST));
            state.shown = None;
        }
        let pending =
            state.pending.is_some() || (state.prepared.is_none() && state.metadata_ready());
        self.jxr_export = Some(state);
        if pending {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
    fn choose_jxr_destination(&mut self, ctx: &egui::Context) {
        let ready = self.jxr_export.as_ref().is_some_and(|s| {
            s.current(self)
                && s.metadata_ready()
                && s.prepared.is_some()
                && s.texture.is_some()
                && s.shown
                    .is_some_and(|frame| frame < ctx.cumulative_frame_nr())
        });
        if !ready {
            self.export_jxr_error(stale());
            return;
        }
        self.jxr_export.as_mut().unwrap().reviewed = true;
        let suggested = Path::new(&self.path_text).with_extension("jxr");
        if let Some(dialogs) = &mut self.dialogs {
            if dialogs.pending() {
                self.export_jxr_error(stale());
                return;
            }
            self.confirm = None;
            self.jxr_export_picker = true;
            let wake = ctx.clone();
            dialogs.request_with_filters(
                Kind::Export,
                Some(&suggested),
                &[paintcraft_platform::dialogs::Filter {
                    name: t!(ctx, "JPEG XR picture"),
                    extensions: vec!["jxr".into()],
                }],
                |text| t!(ctx, text),
                move || wake.request_repaint(),
            );
        } else {
            self.complete_jxr_picker(Some(suggested));
        }
    }
    pub(super) fn complete_jxr_picker(&mut self, path: Option<PathBuf>) {
        let Some(mut state) = self.jxr_export.take() else {
            return;
        };
        self.confirm = None;
        let Some(path) = path else {
            return;
        };
        let result = (|| {
            if !state.current(self)
                || !state.reviewed
                || state.texture.is_none()
                || state.shown.is_none()
            {
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
                        Error::new(
                            ErrorCode::StorageFailed,
                            "Cannot resolve JPEG XR destination",
                        )
                    })?;
                    let source = crate::format_admission::acquire_resource_source(
                        &resolved,
                        encode::ENCODED_BYTES_MAX,
                    )?;
                    Some(save::Expectation::acquired(&resolved, &source.bytes))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(_) => {
                    return Err(Error::new(
                        ErrorCode::StorageFailed,
                        "Cannot inspect JPEG XR destination",
                    ));
                }
            };
            Ok(expected)
        })();
        match result {
            Ok(expected) => {
                state.destination = Some((path, expected));
                self.jxr_export = Some(state);
                self.confirm = Some(Confirm::JxrExport);
            }
            Err(error) => self.export_jxr_error(error),
        }
    }
    pub(super) fn jxr_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.jxr_export.take() else {
            self.confirm = None;
            return;
        };
        if !state.current(self) {
            self.export_jxr_error(stale());
            return;
        }
        let Some((path, expected)) = &state.destination else {
            let before = state.omit_source_metadata;
            let response =
                egui::Modal::new(egui::Id::unique("jxr-export-progress")).show(ctx, |ui| {
                    ui.heading(t!(ctx, "Export image"));
                    if state.has_source_metadata() {
                        paintcraft_ui::source_metadata_export::controls(
                            ui,
                            &mut state.omit_source_metadata,
                        );
                    }
                    if let Some(texture) = &state.texture {
                        show_decoded_preview(ui, texture, &mut state.shown);
                    } else if state.metadata_ready() {
                        ui.spinner();
                        ui.label(t!(ctx, "Preparing JPEG XR picture…"));
                    }
                    let ready = state.metadata_ready()
                        && state.texture.is_some()
                        && state
                            .shown
                            .is_some_and(|frame| frame < ctx.cumulative_frame_nr());
                    ui.horizontal(|ui| {
                        (
                            ui.button(t!(ctx, "Cancel")).clicked(),
                            ui.add_enabled(
                                ready,
                                egui::Button::new(t!(ctx, "Choose destination…")),
                            )
                            .clicked(),
                        )
                    })
                    .inner
                });
            if ctx.current_pass_index() == 0 && (response.inner.0 || response.should_close()) {
                self.confirm = None;
                return;
            }
            if ctx.current_pass_index() == 0 && before != state.omit_source_metadata {
                state.retire_preparation();
                ctx.request_repaint();
            }
            let choose = response.inner.1 && ctx.current_pass_index() == 0;
            self.jxr_export = Some(state);
            if choose {
                self.choose_jxr_destination(ctx);
            }
            return;
        };
        let Some(prepared) = &state.prepared else {
            self.export_jxr_error(stale());
            return;
        };
        let mut message = t!(
            ctx,
            "Export { $path } as { $format }.",
            &[
                ("path", path.display().to_string()),
                ("format", "JPEG XR".into())
            ]
        );
        for loss in &prepared.losses {
            message.push('\n');
            message.push_str(&t!(
                ctx,
                match loss.as_str() {
                    "flatten" => "Visible layers will be flattened into one picture.",
                    "hidden-rgb" => "Invisible color information will not be retained.",
                    export::metadata_policy::OMITTED_REASON =>
                        "Original source metadata is omitted from this exported copy.",
                    _ => loss.as_str(),
                }
            ));
        }
        if expected.is_some() {
            message.push('\n');
            message.push_str(&t!(ctx, "The file exists and will be replaced."));
        }
        let choice = paintcraft_ui::confirmation::show(
            ctx,
            egui::Id::unique("jxr-export-confirm"),
            paintcraft_ui::confirmation::Prompt {
                title: "Export",
                message: &message,
                accept: "Export now",
                discard: None,
            },
        );
        self.jxr_export = Some(state);
        if ctx.current_pass_index() != 0 {
            return;
        }
        match choice {
            Some(paintcraft_ui::confirmation::Choice::Accept) => self.finish_jxr_export(),
            Some(paintcraft_ui::confirmation::Choice::Cancel) => {
                self.jxr_export = None;
                self.confirm = None;
            }
            _ => {}
        }
    }
    fn finish_jxr_export(&mut self) {
        let Some(state) = self.jxr_export.take() else {
            return;
        };
        self.confirm = None;
        let result = (|| {
            if !state.current(self)
                || !state.reviewed
                || state.texture.is_none()
                || state.shown.is_none()
            {
                return Err(stale());
            }
            let capture = state.capture.as_ref().ok_or_else(stale)?;
            let prepared = state.prepared.as_ref().ok_or_else(stale)?;
            let (path, expected) = state.destination.as_ref().ok_or_else(stale)?;
            let check = || {
                state.metadata_policy.validate(
                    &capture.source,
                    Target::new(Family::Jxr, Operation::ExportFlat),
                )?;
                if !state.current(self) || !state.metadata_ready() {
                    return Err(stale());
                }
                admit(self.format_frontend(), path)?;
                self.guard_export_destination(path)?;
                capture.sources.check_destination(path)
            };
            check()?;
            let outcome = save::save_with_expectation_and_preparation(
                path,
                expected.is_some(),
                encode::ENCODED_BYTES_MAX,
                expected.as_ref(),
                |sink| {
                    std::io::Write::write_all(sink, prepared.encoded.bytes()).map_err(|_| {
                        Error::new(ErrorCode::StorageFailed, "Cannot stage JPEG XR output")
                    })
                },
                |_| {
                    prepared
                        .encoded
                        .check_current(&check, &|| Ok(self.editor.native_import_owner()))
                },
            );
            Ok(outcome)
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
            Err(error) => self.export_jxr_error(error),
        }
    }
    fn export_jxr_error(&mut self, error: Error) {
        self.jxr_export = None;
        if self.confirm == Some(Confirm::JxrExport) {
            self.confirm = None;
        }
        let detail = if error.message.starts_with("JXR RGB8 export requires") {
            t!(
                &self.context,
                "JPEG XR export requires an entirely opaque picture."
            )
        } else if error.message == "Finish the current editing operation before exporting a picture"
        {
            t!(
                &self.context,
                "Finish the current editing operation before exporting a picture."
            )
        } else if error.message == "Background image export is unavailable" {
            t!(&self.context, "Background image export is unavailable.")
        } else if error.code == ErrorCode::ConflictStaleBase {
            t!(
                &self.context,
                "The document changed; choose the image again."
            )
        } else {
            error.to_string()
        };
        self.editor.set_status(format!("Cannot export: {detail}"));
    }
}
/// Credit only a fully clipped-in image, and only on a later rendered frame.
fn show_decoded_preview(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    shown: &mut Option<u64>,
) -> bool {
    ui.label(t!(ui.ctx(), "Export preview"));
    let rect = ui.image(texture).rect;
    if ui.is_rect_visible(rect) && ui.clip_rect().contains_rect(rect) {
        shown.get_or_insert(ui.ctx().cumulative_frame_nr());
    }
    shown.is_some_and(|frame| frame < ui.ctx().cumulative_frame_nr())
}

fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "The document changed; choose the image again.",
    )
}

#[cfg(test)]
#[path = "jxr_export/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "jxr_export/installed_tests.rs"]
mod installed_tests;
