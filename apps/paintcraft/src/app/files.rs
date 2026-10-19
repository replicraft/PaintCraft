//! Desktop files integration.

use super::*;

impl App {
    pub(super) fn request_dialog(&mut self, ctx: &egui::Context, kind: Kind) {
        self.editor.cancel_gesture();
        self.editor.commit_text();
        let suggested = if kind == Kind::Export {
            PathBuf::from("untitled.png")
        } else {
            PathBuf::from(&self.path_text)
        };
        if let Some(dialogs) = &mut self.dialogs {
            let ctx = ctx.clone();
            dialogs.request(kind, Some(&suggested), move || ctx.request_repaint());
        }
    }

    pub(super) fn poll_dialog(&mut self, ctx: &egui::Context) {
        let Some(completion) = self.dialogs.as_mut().and_then(NativeDialogs::poll) else {
            return;
        };
        let Some(path) = completion.path else {
            return;
        };
        self.path_text = path.to_string_lossy().into_owned();
        match completion.kind {
            Kind::Open => self.open_selected(path),
            Kind::SaveAs => self.save_selected(ctx, path),
            Kind::Export => self.export_selected(path),
        }
    }

    /// Swap in a new editor; its predecessor's work was already saved or deliberately discarded.
    pub(super) fn prepare_editor_replacement(&mut self) -> paintcraft_core::types::Result<()> {
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
        self.prepare_editor_replacement()?;
        self.publish_editor(editor);
        Ok(())
    }

    pub(super) fn publish_editor(&mut self, editor: Editor) {
        if let Some((_, handle)) = self.pending_frame.take()
            && let Some(renderer) = &self.renderer
        {
            renderer.cancel(handle);
        }
        if let Some(saving) = &mut self.saving {
            saving.new_session();
        }
        self.editor = editor;
        self.texture_key = None;
        self.texture = None;
        self.rendered.clear();
        self.recovery_needs_init = self.recovery_dir.is_some();
        self.attach_recovery();
    }

    /// Open a `.pcnd` document or import a PNG, BMP or JPEG picture.
    pub(super) fn open_path(&mut self, path: &Path) -> paintcraft_core::types::Result<()> {
        let bytes = std::fs::read(path).map_err(|e| {
            paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::StorageFailed,
                format!("cannot read {}: {}", path.display(), e.kind()),
            )
        })?;
        let (editor, bound, note) = if image::ImageFormat::sniff(&bytes).is_some() {
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
            let editor = Editor::open(path)?;
            (
                editor,
                Some(path.to_path_buf()),
                format!("Opened {}", path.display()),
            )
        };
        self.install(editor)?;
        self.bound = bound;
        self.editor.set_status(note);
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
        if self.editor.is_dirty() {
            self.confirm = Some(Confirm::DiscardForOpen(path));
        } else if let Err(e) = self.open_path(&path) {
            self.editor.set_status(format!("Cannot open: {e}"));
        }
    }

    /// Plan an export to the format named by the file extension; losses and
    /// replacement are confirmed before anything is written.
    pub(super) fn export_requested(&mut self, ctx: &egui::Context) {
        if self.dialogs.is_some() {
            self.request_dialog(ctx, Kind::Export);
        } else {
            self.export_selected(PathBuf::from(self.path_text.trim()));
        }
    }

    pub(super) fn export_selected(&mut self, path: PathBuf) {
        let Some(format) = ImageFormat::from_extension(&path) else {
            self.editor
                .set_status("Name the file .png, .bmp or .jpg to export");
            return;
        };
        let all: Vec<String> = KNOWN_LOSSES.iter().map(|l| (*l).to_owned()).collect();
        match export::export_flat(self.editor.document(), format, 90, &all) {
            Ok(plan) => {
                self.confirm = Some(Confirm::Export {
                    exists: path.exists(),
                    path,
                    format,
                    losses: plan.losses,
                })
            }
            Err(e) => self.editor.set_status(format!("Cannot export: {e}")),
        }
    }

    pub(super) fn do_export(&mut self, path: &Path, format: ImageFormat, losses: &[String]) {
        let result = export::export_flat(self.editor.document(), format, 90, losses)
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
        if path.exists() && self.bound.as_deref() != Some(path.as_path()) {
            self.confirm = Some(Confirm::ReplaceFile(path));
            return;
        }
        self.do_save(ctx, path);
    }

    pub(super) fn do_save(&mut self, ctx: &egui::Context, path: PathBuf) {
        if let Some(saving) = &mut self.saving {
            let activation = token(ctx, code::SAVE);
            if self.save_activation == Some(activation) {
                return;
            }
            self.save_activation = Some(activation);
            self.editor.cancel_gesture();
            self.editor.commit_text();
            self.editor.run_pending();
            let result = saving.submit(&self.editor, path, self.bound.as_deref(), ctx.clone());
            self.editor.set_status(match result {
                Ok(()) => "Saving captured document…".into(),
                Err(error) => format!("Save not queued: {error}"),
            });
            return;
        }
        let previous = self.editor.path().map(Path::to_path_buf);
        self.editor.set_path(path.clone());
        self.request(ctx, code::SAVE, Action::Save);
        self.pending_save = Some((path, previous));
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
            self.bound = Some(path);
            self.protect_recovery_source();
            self.pump_recovery_handoff();
        } else {
            self.editor.restore_path(previous);
        }
    }

    pub(super) fn new_document(&mut self) {
        if let Ok(e) = Editor::new_document(DEFAULT_SIZE.0, DEFAULT_SIZE.1) {
            match self.install(e) {
                Ok(()) => self.bound = None,
                Err(error) => self
                    .editor
                    .set_status(format!("Cannot replace document: {error}")),
            }
        }
    }
}
