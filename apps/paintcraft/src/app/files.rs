//! Desktop files integration.

use super::*;
use paintcraft_ui::t;
impl App {
    pub(super) fn request_dialog(&mut self, ctx: &egui::Context, kind: Kind) {
        self.request_dialog_suggested(ctx, kind, None);
    }

    pub(super) fn request_dialog_suggested(
        &mut self,
        ctx: &egui::Context,
        kind: Kind,
        suggestion: Option<PathBuf>,
    ) {
        if !self.editor.shape_file_ready() {
            return;
        }
        if self.document_count() == 0 && !matches!(kind, Kind::Open | Kind::Import) {
            return;
        }
        self.editor.cancel_gesture();
        if kind != Kind::Export {
            self.editor.commit_text();
        }
        let suggested = suggestion.unwrap_or_else(|| {
            if kind == Kind::Export {
                PathBuf::from("untitled.png")
            } else {
                PathBuf::from(&self.path_text)
            }
        });
        let owner = self.active_tab_id();
        let mut filters = self.format_filters(kind);
        if kind == Kind::Export
            && let Some(extension) = suggested
                .extension()
                .and_then(|extension| extension.to_str())
            && let Some(index) = filters.iter().position(|filter| {
                filter
                    .extensions
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(extension))
            })
        {
            let selected = filters.remove(index);
            filters.insert(0, selected);
        }
        let frontend = self.format_frontend();
        let source = (matches!(kind, Kind::Open | Kind::Import))
            .then(|| (self.editor.document_snapshot(), self.editor.render_key()));
        if let Some(dialogs) = &mut self.dialogs
            && !dialogs.pending()
        {
            self.dialog_owner = (owner != Uuid::nil()).then_some(owner);
            self.dialog_frontend = Some(frontend);
            self.dialog_source = source;
            if kind == Kind::Export {
                self.export_dialog_stamp = Some(self.editor.preview_stamp());
            }
            let wake_ctx = ctx.clone();
            dialogs.request_with_filters(
                kind,
                Some(&suggested),
                &filters,
                |source| t!(ctx, source),
                move || wake_ctx.request_repaint(),
            );
        }
    }

    pub(super) fn poll_dialog(&mut self, ctx: &egui::Context) {
        let Some(completion) = self.dialogs.as_mut().and_then(NativeDialogs::poll) else {
            return;
        };
        self.complete_dialog(ctx, completion);
    }

    pub(super) fn complete_dialog(
        &mut self,
        ctx: &egui::Context,
        completion: paintcraft_platform::dialogs::Completion,
    ) {
        if completion.kind == Kind::ImportBrush {
            self.complete_brush_picker(ctx, completion.path);
            return;
        }
        if completion.kind == Kind::ImportSvgFont {
            self.complete_svg_font_picker(ctx, completion.path);
            return;
        }
        if matches!(completion.kind, Kind::ImportShape | Kind::ExportShape) {
            self.complete_shape_picker(ctx, completion.kind, completion.path);
            return;
        }
        if matches!(completion.kind, Kind::ImportPalette | Kind::ExportPalette) {
            self.complete_palette_picker(ctx, completion.kind, completion.path);
            return;
        }
        if completion.kind == Kind::ExportIcns {
            self.complete_icns_picker(ctx, completion.path);
            return;
        }
        if completion.kind == Kind::ExportIcons {
            self.complete_icon_picker(ctx, completion.path);
            return;
        }
        let owner = self.dialog_owner.take();
        let source = self.dialog_source.take();
        let frontend = self.dialog_frontend.take();
        if matches!(completion.kind, Kind::Open | Kind::Import)
            && frontend.is_some_and(|frontend| frontend != self.format_frontend())
        {
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        let jpeg_quality = if completion.kind == Kind::Export {
            self.export_dialog_quality.take().unwrap_or(90)
        } else {
            90
        };
        let pcx_encoding = if completion.kind == Kind::Export {
            self.export_dialog_pcx.take().unwrap_or_default()
        } else {
            Default::default()
        };
        let export_stamp = if completion.kind == Kind::Export {
            self.export_dialog_stamp.take()
        } else {
            None
        };
        if matches!(completion.kind, Kind::Open | Kind::Import)
            && completion.path.is_some()
            && let Some((base, ticket)) = source
        {
            let editor = if owner.is_none_or(|owner| owner == self.active_tab_id()) {
                Some(&self.editor)
            } else {
                self.tabs
                    .iter()
                    .find(|tab| Some(tab.id) == owner)
                    .and_then(|tab| tab.state.as_ref())
                    .map(|state| &state.editor)
            };
            if editor.is_none_or(|editor| {
                !Arc::ptr_eq(&base, &editor.document_snapshot())
                    || ticket != editor.render_key()
                    || editor.has_pending_edits()
                    || editor.gesture_in_progress()
                    || editor.is_editing_text()
            }) {
                self.editor
                    .set_status(t!(ctx, "The document changed; choose the image again."));
                return;
            }
        }
        if (completion.path.is_some() || completion.kind == Kind::InspectRecovery)
            && let Some(owner) = owner
        {
            if !self.tabs.iter().any(|t| t.id == owner) {
                self.editor.set_status(
                    "The file dialog's document is no longer open; its selection was not applied.",
                );
                return;
            }
            self.activate_tab(owner, ctx);
            if self.active_tab_id() != owner {
                self.editor.set_status("The file dialog's document could not be activated; its selection was not applied.");
                return;
            }
        }
        if completion.kind == Kind::InspectRecovery {
            self.recovered_destination_selected(completion.path);
            return;
        }
        let Some(path) = completion.path else {
            return;
        };
        if export_stamp.is_some_and(|stamp| stamp != self.editor.preview_stamp()) {
            self.editor
                .set_status(t!(ctx, "The document changed; choose the image again."));
            return;
        }
        if completion.kind == Kind::SaveAs {
            self.path_text = path.to_string_lossy().into_owned();
        }
        match completion.kind {
            Kind::Open => self.open_selected(path),
            Kind::Import => self.import_selected(path),
            Kind::SaveAs => self.save_selected(ctx, path),
            Kind::Export => self.export_selected_settings(
                path,
                export_options::Settings {
                    jpeg_quality,
                    pcx_encoding,
                },
            ),
            Kind::InspectRecovery => unreachable!("inspection completion handled separately"),
            Kind::ExportIcns | Kind::ExportIcons => {
                unreachable!("icon completion handled separately")
            }
            Kind::ImportBrush => unreachable!("brush completion handled separately"),
            Kind::ImportSvgFont => unreachable!("SVG font completion handled separately"),
            Kind::ImportShape | Kind::ExportShape => {
                unreachable!("shape completion handled separately")
            }
            Kind::ImportPalette | Kind::ExportPalette => {
                unreachable!("palette completion handled separately")
            }
        }
    }

    /// Swap in a new editor; its predecessor's work was already saved or deliberately discarded.
    pub(super) fn prepare_editor_replacement(&mut self) -> paintcraft_core::types::Result<()> {
        if !self.deferred_files.is_empty() {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ResourceLimit,
                "finish queued file requests before replacing this document",
            ));
        }
        if let Some(saving) = &mut self.saving {
            let controls = usize::from(self.editor.recovery_handle().is_some())
                + usize::from(self.recovery_source.is_some());
            saving.reserve_controls(controls)?;
            saving.clear_recovery(&mut self.editor)?;
            if let Some(source) = self.recovery_source.take() {
                saving.discard_source(source)?;
            }
        } else {
            self.editor.discard_checkpoint()?;
            if let Some(source) = self.recovery_source.take() {
                source.discard()?;
            }
        }
        Ok(())
    }

    pub(super) fn install(&mut self, editor: Editor) -> paintcraft_core::types::Result<()> {
        if self.experience() == Experience::Studio || self.tabs.is_empty() {
            return self.add_document_tab(editor, &self.context.clone());
        }
        self.prepare_editor_replacement()?;
        self.publish_editor(editor);
        Ok(())
    }

    pub(super) fn publish_editor(&mut self, editor: Editor) {
        let mut editor = editor.with_text_renderer(self.text_renderer.clone());
        if let Some(service) = &self.editing {
            editor.connect_editing(service.clone());
        }
        if let Some((_, handle)) = self.pending_frame.take()
            && let Some(renderer) = &self.renderer
        {
            renderer.cancel(handle);
        }
        if let Some(saving) = &mut self.saving {
            saving.new_session();
        }
        self.editor = editor;
        self.imported_source = None;
        self.auxiliary_sources.clear();
        self.studio_fit = None;
        self.initial_fit = true;
        self.texture_key = None;
        self.render_region = None;
        self.render_level = 0;
        self.texture = None;
        self.gpu_texture = None;
        self.rendered.clear();
        self.recovery_needs_init = self.recovery_dir.is_some();
        self.attach_recovery();
    }

    /// Open a `.pcnd` document or import a PNG, BMP or JPEG picture.
    pub(super) fn open_path(&mut self, path: &Path) -> paintcraft_core::types::Result<()> {
        self.open_path_with_conversion(path, false)
    }

    fn open_path_with_conversion(
        &mut self,
        path: &Path,
        convert: bool,
    ) -> paintcraft_core::types::Result<()> {
        if convert
            && path
                .extension()
                .and_then(|value| value.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                == Some(paintcraft_formats::registry::Family::Jxl)
        {
            return self.stage_native_jxl(path.to_path_buf());
        }
        if !convert
            && self.experience() == Experience::Studio
            && let Some(id) = self.tab_bound_to_path(path)
        {
            self.activate_tab(id, &self.context.clone());
            self.recent.remember(path);
            return Ok(());
        }
        if convert
            && path
                .extension()
                .and_then(|s| s.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                == Some(paintcraft_formats::registry::Family::Svg)
        {
            if self.format_frontend() != paintcraft_formats::registry::Frontend::Studio {
                return Err(paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                    "SVG rasterization requires explicit Studio import",
                ));
            }
            let source = crate::format_admission::acquire_resource_source(
                path,
                paintcraft_formats::svg::SOURCE_MAX as u64,
            )?;
            return self.stage_svg_import_acquired(source, path.to_path_buf());
        }
        if convert
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                == Some(paintcraft_formats::registry::Family::Msp)
        {
            let source = crate::format_admission::acquire_resource_source(
                path,
                paintcraft_formats::msp::BYTES_MAX as u64,
            )?;
            return self.stage_msp_import_acquired(source, path.to_path_buf());
        }
        if convert
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(paintcraft_formats::registry::Family::from_extension)
                == Some(paintcraft_formats::registry::Family::Icns)
        {
            let source = crate::format_admission::acquire_resource_source(
                path,
                paintcraft_formats::icns::BYTES_MAX as u64,
            )?;
            return self.stage_icns_import_acquired(source, path.to_path_buf());
        }
        let (bytes, binding) = crate::destination::acquire(path)?;
        if convert
            && matches!(
                paintcraft_formats::registry::sniff(&bytes),
                Some(
                    paintcraft_formats::registry::Family::Ico
                        | paintcraft_formats::registry::Family::Cur
                )
            )
        {
            return self.stage_icon_import(bytes, path.to_path_buf());
        }
        if convert
            && paintcraft_formats::registry::sniff(&bytes)
                == Some(paintcraft_formats::registry::Family::Dcx)
        {
            return self.stage_dcx_import(bytes, path.to_path_buf());
        }

        let family = if convert {
            let family = paintcraft_formats::registry::admit_import(
                self.format_frontend(),
                &path.to_string_lossy(),
                &bytes,
                paintcraft_formats::registry::Platform::Native,
            )?;
            if family.is_project() && !paintcraft_formats::project_import::supports(family) {
                return Err(paintcraft_core::types::Error::new(
                    paintcraft_core::types::ErrorCode::CapabilityUnsupported,
                    "Import image expects a raster picture",
                ));
            }
            family
        } else {
            self.admit_open_source(path, &bytes)?
        };
        if paintcraft_formats::project_import::supports(family) {
            let converted = paintcraft_formats::project_import::decode_layered(
                family,
                &bytes,
                4_194_304,
                &|| Ok(()),
            )?;
            return self.stage_project_import(converted, path.to_path_buf());
        }
        let (editor, bound, note) = if family != paintcraft_formats::registry::Family::Pcnd {
            let img = image::decode(&bytes)?;
            if !img.losses.is_empty() {
                return self.stage_import(
                    img,
                    path.to_path_buf(),
                    hex(&Sha256::digest(&bytes)),
                    convert,
                );
            }
            let doc = import_image(
                img.width,
                img.height,
                &img.rgba,
                &hex(&Sha256::digest(&bytes)),
            )?;
            let note = if img.losses.is_empty() {
                format!("Imported {}", path.display())
            } else {
                format!(
                    "Imported {} (not kept: {})",
                    path.display(),
                    img.losses.join(", ")
                )
            };
            // A picture is a new document; it is not bound to the image file.
            (Editor::with_document(doc, None), None, note)
        } else {
            // Decode the same bounded acquisition; do not reopen an unbounded path.
            let mut editor = Editor::with_document(
                paintcraft_formats::pcnd::read(&bytes)?,
                Some(path.to_path_buf()),
            );
            editor.bind_destination(Some(binding));
            (
                editor,
                Some(path.to_path_buf()),
                format!("Opened {}", path.display()),
            )
        };
        self.install(editor)?;
        self.bound = bound;
        self.imported_source = self.bound.is_none().then(|| path.to_path_buf());
        self.path_text = if self.bound.is_some() {
            path.to_path_buf()
        } else {
            path.with_extension("pcnd")
        }
        .to_string_lossy()
        .into_owned();
        self.editor.set_status(note);
        if !convert {
            self.recent.remember(path);
        }
        Ok(())
    }

    pub(super) fn open_requested(&mut self, ctx: &egui::Context) {
        if self.dialogs.is_some() {
            self.request_dialog(ctx, Kind::Open);
        } else {
            self.open_selected(PathBuf::from(self.path_text.trim()));
        }
    }

    pub(super) fn open_selected(&mut self, path: PathBuf) {
        if self.experience() == Experience::Classic && self.editor.is_dirty() {
            self.confirm = Some(Confirm::DiscardForOpen(path));
        } else if let Err(e) = self.open_path(&path) {
            self.editor.set_status(format!("Cannot open: {e}"));
        }
    }

    pub(super) fn import_selected(&mut self, path: PathBuf) {
        if let Err(error) = self.open_path_with_conversion(&path, true) {
            self.editor.set_status(format!("Cannot open: {error}"));
        }
    }

    /// Stage format and encoder options before opening the native destination picker.
    pub(super) fn export_requested(&mut self, _ctx: &egui::Context) {
        self.begin_export_options(paintcraft_ui::actions::ImageExport::Png);
    }

    pub(super) fn export_image_requested(
        &mut self,
        _ctx: &egui::Context,
        format: paintcraft_ui::actions::ImageExport,
    ) {
        self.begin_export_options(format);
    }

    #[cfg(test)]
    pub(super) fn export_selected(&mut self, path: PathBuf) {
        self.export_selected_quality(path, 90);
    }

    #[cfg(test)]
    pub(super) fn export_selected_quality(&mut self, path: PathBuf, jpeg_quality: u8) {
        self.export_selected_settings(
            path,
            export_options::Settings {
                jpeg_quality,
                ..Default::default()
            },
        );
    }

    pub(super) fn export_selected_settings(
        &mut self,
        path: PathBuf,
        settings: export_options::Settings,
    ) {
        if !self.editor.shape_file_ready() {
            return;
        }
        self.editor.commit_text();
        self.editor.run_pending();
        if self.editing.is_some() {
            self.defer_file(
                editing::FileRequest::Export(path, settings),
                token(&self.context, code::EXPORT),
            );
            return;
        }
        self.plan_export_settings(path, settings);
    }

    pub(super) fn plan_export_settings(
        &mut self,
        path: PathBuf,
        settings: export_options::Settings,
    ) {
        let Some(format) = ImageFormat::from_extension(&path) else {
            self.editor
                .set_status("Choose a supported image filename extension.");
            return;
        };
        if let Err(error) = self.admit_export_format(&path, format) {
            self.editor.set_status(format!("Cannot export: {error}"));
            return;
        }
        let all: Vec<String> = KNOWN_LOSSES.iter().map(|l| (*l).to_owned()).collect();
        match export::export_flat_options(self.editor.document(), settings.options(format), &all) {
            Ok(plan) => {
                self.confirm = Some(Confirm::Export {
                    exists: path.exists(),
                    path,
                    format,
                    losses: plan.losses,
                    jpeg_quality: settings.jpeg_quality,
                    pcx_encoding: settings.pcx_encoding,
                    source: self.editor.preview_stamp(),
                })
            }
            Err(e) => self.editor.set_status(format!("Cannot export: {e}")),
        }
    }

    #[cfg(test)]
    pub(super) fn do_export(&mut self, path: &Path, format: ImageFormat, losses: &[String]) {
        self.do_export_quality(path, format, losses, 90);
    }

    #[cfg(test)]
    pub(super) fn do_export_quality(
        &mut self,
        path: &Path,
        format: ImageFormat,
        losses: &[String],
        jpeg_quality: u8,
    ) {
        self.do_export_settings(
            path,
            format,
            losses,
            export_options::Settings {
                jpeg_quality,
                ..Default::default()
            },
        );
    }

    pub(super) fn do_export_settings(
        &mut self,
        path: &Path,
        format: ImageFormat,
        losses: &[String],
        settings: export_options::Settings,
    ) {
        if let Err(error) = self.admit_export_format(path, format) {
            self.editor.set_status(format!("Cannot export: {error}"));
            return;
        }
        let result =
            export::export_flat_options(self.editor.document(), settings.options(format), losses)
                .map_err(|e| e.to_string());
        let status = match result {
            Ok(flat) => {
                let outcome = save::save(path, &flat.bytes, true);
                if outcome.is_success() {
                    format!("Exported {}", path.display())
                } else {
                    format!("Not exported safely: {}", outcome.reasons.join("; "))
                }
            }
            Err(e) => format!("Cannot export: {e}"),
        };
        self.editor.set_status(status);
    }

    pub(super) fn save_requested(&mut self, ctx: &egui::Context) {
        if !self.editor.shape_file_ready() {
            return;
        }
        if self.document_count() == 0 {
            return;
        }
        if self.dialogs.is_some() && self.bound.is_none() {
            self.request_dialog(ctx, Kind::SaveAs);
            return;
        }
        let path = self
            .bound
            .clone()
            .unwrap_or_else(|| PathBuf::from(self.path_text.trim()));
        self.save_selected(ctx, path);
    }

    pub(super) fn save_selected(&mut self, ctx: &egui::Context, path: PathBuf) {
        if path.as_os_str().is_empty() {
            return;
        }
        if let Err(error) = self.guard_save_destination(&path) {
            self.editor.set_status(format!("Cannot save: {error}"));
            return;
        }
        if self.other_document_destination(&path) {
            self.confirm = Some(Confirm::ReplaceSharedFile(path));
            return;
        }
        if path.exists() && self.bound.as_deref() != Some(path.as_path()) {
            self.confirm = Some(Confirm::ReplaceFile(path));
            return;
        }
        self.do_save(ctx, path);
    }

    pub(super) fn do_save(&mut self, ctx: &egui::Context, path: PathBuf) {
        if let Err(error) = self.guard_save_destination(&path) {
            self.editor.set_status(format!("Cannot save: {error}"));
            return;
        }
        if !self.editor.shape_file_ready() {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.commit_text();
        self.editor.run_pending();
        if self.editing.is_some() {
            self.defer_file(editing::FileRequest::Save(path), token(ctx, code::SAVE));
            return;
        }
        self.capture_save(ctx, path);
    }

    pub(super) fn capture_save(&mut self, ctx: &egui::Context, path: PathBuf) {
        // Recheck after a confirmation or deferred editor transaction. The
        // destination may have become an alias of a loaded resource meanwhile.
        if let Err(error) = self.guard_save_destination(&path) {
            self.editor.set_status(format!("Cannot save: {error}"));
            return;
        }
        if let Some((id, source)) = &mut self.tab_save_close
            && self
                .tabs
                .get(self.active_tab)
                .is_some_and(|tab| tab.id == *id)
        {
            *source = self.editor.document().state_id;
        }
        let activation = token(ctx, code::SAVE);
        if self.save_activation == Some(activation) {
            return;
        }
        self.save_activation = Some(activation);
        if let Some(saving) = &mut self.saving {
            let result = saving.submit(&self.editor, path, self.bound.as_deref(), ctx.clone());
            self.editor.set_status(match result {
                Ok(()) => "Saving captured document…".into(),
                Err(error) => format!("Save not queued: {error}"),
            });
            return;
        }
        let previous = self.editor.file_association();
        self.editor.set_path(path.clone());
        self.pending_save = Some((path, previous));
        if self.editor.at_file_barrier() {
            self.editor.save_captured();
            self.finish_save();
        } else {
            self.request(ctx, code::SAVE, Action::Save);
        }
    }

    pub(super) fn finish_save(&mut self) {
        let Some(success) = self.editor.take_save_outcome() else {
            return;
        };
        let Some((path, previous)) = self.pending_save.take() else {
            return;
        };
        if success {
            self.path_text = path.to_string_lossy().into_owned();
            self.bound = Some(path.clone());
            self.protect_recovery_source();
            self.pump_recovery_handoff();
            let snapshot = self.editor.document_snapshot();
            self.destination_written(&path, &snapshot, Some(self.active_tab_id()));
        } else {
            self.editor.restore_path(previous);
        }
    }

    pub(super) fn new_document(&mut self) {
        let size = if self.experience() == Experience::Classic {
            self.settings.canvas.new_size()
        } else {
            STUDIO_SIZE
        };
        if let Ok(e) = Editor::new_document(size.0, size.1) {
            match self.install(e) {
                Ok(()) => {
                    self.bound = None;
                    if self.experience() == Experience::Classic {
                        self.remember_canvas_size(size);
                    }
                }
                Err(error) => self
                    .editor
                    .set_status(format!("Cannot replace document: {error}")),
            }
        }
    }
}
