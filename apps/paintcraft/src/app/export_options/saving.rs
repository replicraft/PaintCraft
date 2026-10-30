//! Frozen, document-owned generic export copies.
use super::*;
impl App {
    /// Stage format and encoder options before opening the native destination picker.
    pub(crate) fn export_requested(&mut self, _ctx: &egui::Context) {
        self.begin_export_options(paintcraft_ui::actions::ImageExport::Png);
    }

    pub(crate) fn export_image_requested(
        &mut self,
        _ctx: &egui::Context,
        format: paintcraft_ui::actions::ImageExport,
    ) {
        self.begin_export_options(format);
    }

    #[cfg(test)]
    pub(crate) fn export_selected(&mut self, path: PathBuf) {
        self.export_selected_quality(path, 90);
    }

    #[cfg(test)]
    pub(crate) fn export_selected_quality(&mut self, path: PathBuf, jpeg_quality: u8) {
        self.export_selected_settings(
            path,
            export_options::Settings {
                jpeg_quality,
                ..Default::default()
            },
        );
    }

    pub(crate) fn export_selected_settings(
        &mut self,
        path: PathBuf,
        settings: export_options::Settings,
    ) {
        if !self.editor.shape_file_ready() {
            return;
        }
        self.editor.run_pending();
        if (self.editing.is_some() && !self.editor.failed_text_retained())
            || !self.editor.can_capture_export()
        {
            if self.editor.failed_text_retained() {
                return;
            }
            self.defer_file(
                editing::FileRequest::Export(path, settings),
                token(&self.context, code::EXPORT),
            );
            return;
        }
        self.plan_export_settings(path, settings);
    }

    pub(crate) fn plan_export_settings(
        &mut self,
        path: PathBuf,
        settings: export_options::Settings,
    ) {
        match Frozen::capture(self) {
            Ok(frozen) => self.plan_export_frozen(path, settings, frozen),
            Err(error) => self.editor.set_status(format!("Cannot export: {error}")),
        }
    }

    pub(crate) fn plan_export_frozen(&mut self, path: PathBuf, settings: Settings, frozen: Frozen) {
        if !frozen.current(self) {
            return;
        }
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
        let target = Target::new(
            paintcraft_formats::registry::Family::from_id(format.name())
                .expect("registered image format"),
            paintcraft_formats::registry::Operation::ExportFlat,
        );
        let policy = match Policy::capture(&frozen.document, target, settings.source_metadata) {
            Ok(policy) => policy,
            Err(error) => {
                self.editor.set_status(format!("Cannot export: {error}"));
                return;
            }
        };
        let mut options = settings.options(format);
        options.source_metadata = policy;
        options.filter_bake = frozen.filter_bake;
        match export::export_flat_options(&frozen.document, options, &all) {
            Ok(plan) => {
                self.confirm = Some(Confirm::Export {
                    exists: path.exists(),
                    path,
                    format,
                    losses: plan.losses,
                    jpeg_quality: settings.jpeg_quality,
                    jpeg_matte: settings.jpeg_matte,
                    pcx_encoding: settings.pcx_encoding,
                    source: self.editor.preview_stamp(),
                    frozen,
                    metadata_policy: policy,
                })
            }
            Err(e) => self.editor.set_status(format!("Cannot export: {e}")),
        }
    }

    #[cfg(test)]
    pub(crate) fn do_export(&mut self, path: &Path, format: ImageFormat, losses: &[String]) {
        self.do_export_quality(path, format, losses, 90);
    }

    #[cfg(test)]
    pub(crate) fn do_export_quality(
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

    #[cfg(test)]
    pub(crate) fn do_export_settings(
        &mut self,
        path: &Path,
        format: ImageFormat,
        losses: &[String],
        settings: export_options::Settings,
    ) {
        let frozen = match Frozen::capture(self) {
            Ok(frozen) => frozen,
            Err(error) => {
                self.editor.set_status(format!("Cannot export: {error}"));
                return;
            }
        };
        let target = Target::new(
            paintcraft_formats::registry::Family::from_id(format.name())
                .expect("registered image format"),
            paintcraft_formats::registry::Operation::ExportFlat,
        );
        let policy = match Policy::capture(&frozen.document, target, settings.source_metadata) {
            Ok(policy) => policy,
            Err(error) => {
                self.editor.set_status(format!("Cannot export: {error}"));
                return;
            }
        };
        self.do_export_frozen(path, format, losses, settings, frozen, policy);
    }

    pub(crate) fn do_export_frozen(
        &mut self,
        path: &Path,
        format: ImageFormat,
        losses: &[String],
        settings: Settings,
        frozen: Frozen,
        policy: Policy,
    ) {
        if !frozen.current(self) {
            self.editor
                .set_status("The document changed; choose the image again.");
            return;
        }
        if let Err(error) = self.admit_export_format(path, format) {
            self.editor.set_status(format!("Cannot export: {error}"));
            return;
        }
        let mut options = settings.options(format);
        options.source_metadata = policy;
        options.filter_bake = frozen.filter_bake;
        let result = export::export_flat_options(&frozen.document, options, losses)
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
}
