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
        if self.document_count() == 0 && kind != Kind::Open {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.commit_text();
        let suggested = suggestion.unwrap_or_else(|| {
            if kind == Kind::Export {
                PathBuf::from("untitled.png")
            } else {
                PathBuf::from(&self.path_text)
            }
        });
        let owner = self.active_tab_id();
        if let Some(dialogs) = &mut self.dialogs
            && !dialogs.pending()
        {
            self.dialog_owner = (owner != Uuid::nil()).then_some(owner);
            let wake_ctx = ctx.clone();
            dialogs.request_with_labels(
                kind,
                Some(&suggested),
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
        let owner = self.dialog_owner.take();
        let jpeg_quality = if completion.kind == Kind::Export {
            self.export_dialog_quality.take().unwrap_or(90)
        } else {
            90
        };
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
        if completion.kind == Kind::SaveAs {
            self.path_text = path.to_string_lossy().into_owned();
        }
        match completion.kind {
            Kind::Open => self.open_selected(path),
            Kind::SaveAs => self.save_selected(ctx, path),
            Kind::Export => self.export_selected_quality(path, jpeg_quality),
            Kind::InspectRecovery => unreachable!("inspection completion handled separately"),
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
        if self.experience() == Experience::Studio
            && let Some(id) = self.tab_bound_to_path(path)
        {
            self.activate_tab(id, &self.context.clone());
            self.recent.remember(path);
            return Ok(());
        }
        let bytes = crate::format_admission::read_source(path)?;
        let family = self.admit_open_source(path, &bytes)?;
        let (editor, bound, note) = if family != paintcraft_formats::registry::Family::Pcnd {
            let img = image::decode(&bytes)?;
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
            let editor = Editor::with_document(
                paintcraft_formats::pcnd::read(&bytes)?,
                Some(path.to_path_buf()),
            );
            (
                editor,
                Some(path.to_path_buf()),
                format!("Opened {}", path.display()),
            )
        };
        self.install(editor)?;
        self.bound = bound;
        self.path_text = if self.bound.is_some() {
            path.to_path_buf()
        } else {
            path.with_extension("pcnd")
        }
        .to_string_lossy()
        .into_owned();
        self.editor.set_status(note);
        self.recent.remember(path);
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

    pub(super) fn export_selected_quality(&mut self, path: PathBuf, jpeg_quality: u8) {
        if !self.editor.shape_file_ready() {
            return;
        }
        self.editor.commit_text();
        self.editor.run_pending();
        if self.editing.is_some() {
            self.defer_file(
                editing::FileRequest::Export(path, jpeg_quality),
                token(&self.context, code::EXPORT),
            );
            return;
        }
        self.plan_export_quality(path, jpeg_quality);
    }

    pub(super) fn plan_export_quality(&mut self, path: PathBuf, jpeg_quality: u8) {
        let Some(format) = ImageFormat::from_extension(&path) else {
            self.editor
                .set_status("Name the file .png, .bmp or .jpg to export");
            return;
        };
        if let Err(error) = self.admit_export_format(&path, format) {
            self.editor.set_status(format!("Cannot export: {error}"));
            return;
        }
        let all: Vec<String> = KNOWN_LOSSES.iter().map(|l| (*l).to_owned()).collect();
        match export::export_flat(self.editor.document(), format, jpeg_quality, &all) {
            Ok(plan) => {
                self.confirm = Some(Confirm::Export {
                    exists: path.exists(),
                    path,
                    format,
                    losses: plan.losses,
                    jpeg_quality,
                })
            }
            Err(e) => self.editor.set_status(format!("Cannot export: {e}")),
        }
    }

    #[cfg(test)]
    pub(super) fn do_export(&mut self, path: &Path, format: ImageFormat, losses: &[String]) {
        self.do_export_quality(path, format, losses, 90);
    }

    pub(super) fn do_export_quality(
        &mut self,
        path: &Path,
        format: ImageFormat,
        losses: &[String],
        jpeg_quality: u8,
    ) {
        if let Err(error) = self.admit_export_format(path, format) {
            self.editor.set_status(format!("Cannot export: {error}"));
            return;
        }
        let result = export::export_flat(self.editor.document(), format, jpeg_quality, losses)
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
        let previous = self.editor.path().map(Path::to_path_buf);
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
