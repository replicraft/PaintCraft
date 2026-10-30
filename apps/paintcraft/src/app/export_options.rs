//! Staged image encoder choices, followed by a native destination picker.
use super::*;
use paintcraft_ui::pcx_export::{self, Encoding};
use paintcraft_ui::{actions::ImageExport, t};
#[path = "export_options/choice.rs"]
mod choice;
use choice::Choice;
#[path = "export_options/frozen.rs"]
mod frozen;
#[path = "export_options/saving.rs"]
mod saving;
use export::metadata_policy::{Intent, Policy, Target};
pub(crate) use frozen::Frozen;
#[path = "export_options/asset_copy.rs"]
mod asset_copy;
pub(crate) use asset_copy::{AssetCopy, other_losses};
#[cfg(test)]
#[path = "export_options/palette_tests.rs"]
mod palette_tests;

#[cfg(test)]
#[path = "export_options/jxl_tests.rs"]
mod jxl_tests;
#[cfg(test)]
#[path = "export_options/matte_tests.rs"]
mod matte_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    pub jpeg_quality: u8,
    pub jpeg_matte: Option<paintcraft_formats::image::JpegMatte>,
    pub pcx_encoding: Encoding,
    pub source_metadata: Intent,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            jpeg_quality: 90,
            jpeg_matte: None,
            pcx_encoding: Encoding::Rgb24,
            source_metadata: Intent::Refuse,
        }
    }
}
impl Settings {
    pub(crate) fn options(self, format: ImageFormat) -> export::FlatOptions {
        let mut options = export::FlatOptions::new(format, self.jpeg_quality);
        options.jpeg_matte = self.jpeg_matte;
        if format == ImageFormat::Pcx {
            options.pcx_mode = mode(self.pcx_encoding);
        }
        options
    }
}
fn mode(encoding: Encoding) -> paintcraft_formats::pcx::Mode {
    match encoding {
        Encoding::Rgb24 => paintcraft_formats::pcx::Mode::Rgb24,
        Encoding::IndexedExact => paintcraft_formats::pcx::Mode::IndexedExact,
        Encoding::Indexed256 => paintcraft_formats::pcx::Mode::Quantized256,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    owner: Uuid,
    frozen: Frozen,
    omit_source_metadata: bool,
    bake_filters: bool,
    format: Choice,
    jpeg_quality: u8,
    jpeg_matte: [u8; 3],
    pcx_encoding: Encoding,
    dds: paintcraft_ui::dds_export::Settings,
    msp_version: paintcraft_ui::msp_export::Version,
}

impl App {
    fn enabled_export_formats(&self) -> Vec<ImageExport> {
        ImageExport::ALL
            .into_iter()
            .filter(|format| {
                let path = PathBuf::from("picture").with_extension(format.extension());
                format.available(self.action_context().frontend)
                    && ImageFormat::from_extension(&path)
                        .is_some_and(|codec| self.admit_export_format(&path, codec).is_ok())
            })
            .collect()
    }
    fn enabled_export_choices(&self) -> Vec<Choice> {
        let mut choices: Vec<_> = self
            .enabled_export_formats()
            .into_iter()
            .map(Choice::Flat)
            .collect();
        if crate::jxr_export::admit(self.format_frontend(), Path::new("picture.jxr")).is_ok() {
            choices.push(Choice::Jxr);
        }
        choices
    }
    pub(super) fn begin_export_options(&mut self, format: ImageExport) {
        if self.document_count() == 0
            || !self.editor.shape_file_ready()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            return;
        }
        if !self.enabled_export_formats().contains(&format) {
            return;
        }
        self.editor.cancel_gesture();
        self.editor.run_pending();
        if !self.editor.can_capture_export() {
            if self.editor.failed_text_retained() {
                return;
            }
            self.defer_file(
                editing::FileRequest::ExportOptions(format),
                token(&self.context, code::EXPORT),
            );
            return;
        }
        let frozen = match Frozen::capture(self) {
            Ok(frozen) => frozen,
            Err(error) => {
                self.editor.set_status(format!("Cannot export: {error}"));
                return;
            }
        };
        self.confirm = Some(Confirm::ExportOptions(Draft {
            owner: self.active_tab_id(),
            frozen,
            omit_source_metadata: false,
            bake_filters: false,
            format: Choice::Flat(format),
            jpeg_quality: 90,
            jpeg_matte: [255; 3],
            pcx_encoding: Encoding::Rgb24,
            dds: Default::default(),
            msp_version: Default::default(),
        }));
    }

    pub(super) fn export_options_dialog(&mut self, ctx: &egui::Context, mut draft: Draft) {
        if draft.owner != self.active_tab_id() || !draft.frozen.current(self) {
            self.confirm = None;
            self.pcx_preview.clear();
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("image-export-options")).show(ctx, |ui| {
            let mut preview_ready = true;
            ui.set_width(380.0);
            ui.heading(t!(ctx, "Export image"));
            ui.label(t!(
                ctx,
                "Choose an image format before selecting where to save."
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let label = ui.label(t!(ctx, "File format"));
                egui::ComboBox::from_id_salt("export-format")
                    .selected_text(t!(ctx, draft.format.label()))
                    .show_ui(ui, |ui| {
                        for format in self.enabled_export_choices() {
                            ui.selectable_value(&mut draft.format, format, t!(ctx, format.label()));
                        }
                    })
                    .response
                    .labelled_by(label.id);
            });
            ui.label(t!(ctx, draft.format.description()));
            if draft.format == Choice::Flat(ImageExport::Dds)
                && crate::dds_export::admit(self.format_frontend(), Path::new("picture.dds"))
                    .is_ok()
            {
                let size = self.editor.document().canvas;
                paintcraft_ui::dds_export::controls(ui, &mut draft.dds, size.0, size.1);
            }
            if draft.format == Choice::Flat(ImageExport::Msp)
                && self.experience() == Experience::Studio
            {
                paintcraft_ui::msp_export::controls(ui, &mut draft.msp_version);
            }
            if draft.format == Choice::Flat(ImageExport::Pcx) {
                pcx_export::controls(ui, &mut draft.pcx_encoding);
                let editor = &self.editor;
                let snapshot = draft.frozen.document.clone();
                preview_ready =
                    self.pcx_preview
                        .show(ui, (editor.preview_stamp(), draft.pcx_encoding), || {
                            let doc = snapshot;
                            let rgba = paintcraft_render::render_flat(&doc)
                                .map_err(|error| error.to_string())?;
                            let prepared = paintcraft_formats::pcx::prepare_rgba(
                                doc.canvas.0,
                                doc.canvas.1,
                                &rgba,
                                mode(draft.pcx_encoding),
                                &|| Ok(()),
                            )
                            .map_err(|error| error.to_string())?;
                            let preview = prepared
                                .preview(192, &|| Ok(()))
                                .map_err(|error| error.to_string())?;
                            Ok(pcx_export::Preview {
                                width: preview.width,
                                height: preview.height,
                                rgba: preview.rgba,
                                quantized: prepared.is_quantized(),
                            })
                        });
            } else {
                self.pcx_preview.clear();
            }
            if let Some(label) = draft.format.quality_label() {
                ui.add_space(8.0);
                ui.add(egui::Slider::new(&mut draft.jpeg_quality, 1..=100).text(t!(ctx, label)));
                ui.label(t!(ctx, if draft.format == Choice::Flat(ImageExport::Avif) {
                    "AVIF uses lossy compression for colors and transparency, even at quality 100."
                } else {
                    "Higher quality creates a larger file."
                }));
            }
            if draft.format == Choice::Flat(ImageExport::Jpeg) {
                paintcraft_ui::jpeg_export::controls(ui, &mut draft.jpeg_matte);
            }
            let separate_review = draft.format == Choice::Jxr
                || (draft.format == Choice::Flat(ImageExport::Msp)
                    && self.experience() == Experience::Studio)
                || (draft.format == Choice::Flat(ImageExport::Dds)
                    && crate::dds_export::advanced(draft.dds, draft.frozen.document.canvas)
                    && crate::dds_export::admit(self.format_frontend(), Path::new("picture.dds"))
                        .is_ok());
            if !separate_review && draft.frozen.document.source_metadata.is_some() {
                paintcraft_ui::source_metadata_export::controls(
                    ui,
                    &mut draft.omit_source_metadata,
                );
                preview_ready &= draft.omit_source_metadata;
            }
            if !separate_review && draft.frozen.has_filters {
                paintcraft_ui::filter_bake_export::controls(ui, &mut draft.bake_filters);
                preview_ready &= draft.bake_filters;
            }
            let canvas = draft.frozen.document.canvas;
            ui.add_space(8.0);
            ui.label(t!(
                ctx,
                "Image dimensions: { $width } × { $height } px",
                &[
                    ("width", canvas.0.to_string()),
                    ("height", canvas.1.to_string()),
                ]
            ));
            ui.label(t!(
                ctx,
                "Save a PaintCraft document to keep editable layers."
            ));
            ui.separator();
            ui.horizontal(|ui| {
                let cancel = ui.button(t!(ctx, "Cancel")).clicked();
                let accept = ui
                    .add_enabled(
                        preview_ready,
                        egui::Button::new(t!(
                            ctx,
                            if draft.format == Choice::Jxr
                                || (draft.format == Choice::Flat(ImageExport::Msp)
                                    && self.experience() == Experience::Studio)
                                || draft.format == Choice::Flat(ImageExport::Dds)
                                    && crate::dds_export::advanced(
                                        draft.dds,
                                        self.editor.document().canvas
                                    )
                            {
                                "Preview export"
                            } else {
                                "Choose destination…"
                            }
                        )),
                    )
                    .clicked();
                (cancel, accept)
            })
            .inner
        });
        if ctx.current_pass_index() != 0 {
            return;
        }
        let (cancel, accept) = response.inner;
        if cancel || response.should_close() {
            self.confirm = None;
            self.pcx_preview.clear();
        } else if accept {
            self.confirm = None;
            self.pcx_preview.clear();
            if draft.format == Choice::Flat(ImageExport::Dds)
                && crate::dds_export::advanced(draft.dds, self.editor.document().canvas)
                && crate::dds_export::admit(self.format_frontend(), Path::new("picture.dds"))
                    .is_ok()
            {
                self.begin_dds_export_frozen(
                    draft.dds,
                    draft.frozen.clone(),
                    if draft.omit_source_metadata {
                        Intent::Omit
                    } else {
                        Intent::Refuse
                    },
                );
            } else if draft.format == Choice::Flat(ImageExport::Msp)
                && self.experience() == Experience::Studio
            {
                self.begin_msp_export_frozen(
                    draft.msp_version,
                    draft.frozen.clone(),
                    if draft.omit_source_metadata {
                        Intent::Omit
                    } else {
                        Intent::Refuse
                    },
                );
            } else if draft.format == Choice::Jxr {
                self.begin_jxr_export_frozen(
                    draft.frozen.clone(),
                    if draft.omit_source_metadata {
                        Intent::Omit
                    } else {
                        Intent::Refuse
                    },
                );
            } else {
                let family =
                    paintcraft_formats::registry::Family::from_extension(draft.format.extension())
                        .expect("registered flat family");
                let target =
                    Target::new(family, paintcraft_formats::registry::Operation::ExportFlat);
                draft.frozen.filter_bake = match export::filter_bake::Policy::capture(
                    &draft.frozen.document,
                    target,
                    if draft.bake_filters {
                        export::filter_bake::Intent::Bake
                    } else {
                        export::filter_bake::Intent::Refuse
                    },
                    &|| Ok(()),
                ) {
                    Ok(policy) => policy,
                    Err(error) => {
                        self.editor.set_status(format!("Cannot export: {error}"));
                        return;
                    }
                };
                if self.dialogs.is_some() {
                    let suggested = self
                        .bound
                        .as_deref()
                        .unwrap_or_else(|| Path::new(&self.path_text))
                        .with_extension(draft.format.extension());
                    self.export_dialog_quality = Some(draft.jpeg_quality);
                    self.export_dialog_matte = (draft.format == Choice::Flat(ImageExport::Jpeg))
                        .then(|| paintcraft_formats::image::JpegMatte::new(draft.jpeg_matte));
                    self.export_dialog_pcx = Some(draft.pcx_encoding);
                    self.export_dialog_frozen = Some(draft.frozen.clone());
                    self.export_dialog_metadata = if draft.omit_source_metadata {
                        Intent::Omit
                    } else {
                        Intent::Refuse
                    };
                    self.request_dialog_suggested(ctx, Kind::Export, Some(suggested));
                } else {
                    // Headless callers supply an explicit destination with set_path_text.
                    let destination =
                        Path::new(&self.path_text).with_extension(draft.format.extension());
                    self.plan_export_frozen(
                        destination,
                        Settings {
                            jpeg_quality: draft.jpeg_quality,
                            jpeg_matte: (draft.format == Choice::Flat(ImageExport::Jpeg)).then(
                                || paintcraft_formats::image::JpegMatte::new(draft.jpeg_matte),
                            ),
                            pcx_encoding: draft.pcx_encoding,
                            source_metadata: if draft.omit_source_metadata {
                                Intent::Omit
                            } else {
                                Intent::Refuse
                            },
                        },
                        draft.frozen,
                    );
                }
            }
        } else {
            self.confirm = Some(Confirm::ExportOptions(draft));
        }
    }
}

#[cfg(test)]
#[path = "export_options/metadata_tests.rs"]
mod metadata_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_export_options_preserves_pixels_history_and_dirty_state() {
        let mut app = App::new(None);
        let snapshot = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        app.begin_export_options(ImageExport::Jpeg);
        assert!(matches!(
            app.confirm,
            Some(Confirm::ExportOptions(Draft {
                format: Choice::Flat(ImageExport::Jpeg),
                jpeg_quality: 90,
                ..
            }))
        ));
        assert!(app.editor.document().same_semantics(&snapshot));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert_eq!(app.editor.is_dirty(), dirty);
    }

    #[test]
    fn confirmation_captures_quality_and_actual_encoder_uses_it() {
        let mut app = App::new(None);
        let path = std::env::temp_dir().join(format!("paintcraft-export-{}.jpg", Uuid::new_v4()));
        app.export_selected_quality(path.clone(), 17);
        let Some(Confirm::Export {
            jpeg_quality,
            format,
            losses,
            ..
        }) = app.confirm.take()
        else {
            panic!("export confirmation");
        };
        assert_eq!(jpeg_quality, 17);
        let expected =
            export::export_flat(app.editor.document(), format, jpeg_quality, &losses).unwrap();
        app.do_export_quality(&path, format, &losses, jpeg_quality);
        assert_eq!(std::fs::read(&path).unwrap(), expected.bytes);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn every_localized_export_dialog_can_continue_to_loss_review_and_cancel() {
        use egui_kittest::{Harness, kittest::Queryable};
        use paintcraft_ui::i18n::{Language, set_language, translate};
        for language in Language::ALL {
            for format in ImageExport::ALL {
                let mut app = App::new(None);
                app.settings.experience = Experience::Studio;
                app.settings.language = language;
                app.editor = Editor::new_document(16, 12).unwrap();
                if format == ImageExport::Msp {
                    app.initialize_editing(app.context.clone());
                }
                let path = std::env::temp_dir()
                    .join(format!("paintcraft-export-dialog-{}.pcnd", Uuid::new_v4()));
                app.path_text = path.to_string_lossy().into_owned();
                app.begin_export_options(format);
                if let Some(Confirm::ExportOptions(draft)) = &mut app.confirm {
                    draft.jpeg_quality = 43;
                }
                let mut harness = Harness::builder()
                    .with_max_steps(if format == ImageExport::Msp { 40 } else { 4 })
                    .with_size(egui::vec2(800.0, 600.0))
                    .build_ui_state(
                        move |ui, app: &mut App| {
                            set_language(ui.ctx(), language);
                            if format == ImageExport::Msp {
                                app.ui(ui);
                            } else {
                                app.confirm_dialog(ui);
                            }
                        },
                        app,
                    );
                harness.run();
                harness
                    .get_by_label(&translate(
                        language,
                        if format == ImageExport::Msp {
                            "Preview export"
                        } else {
                            "Choose destination…"
                        },
                    ))
                    .click();
                if format == ImageExport::Msp {
                    // Follow the new native preparation through the same service
                    // as the App; a spinner cannot settle in a dialog-only harness.
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
                    loop {
                        harness.step();
                        if harness
                            .state()
                            .msp_export
                            .as_ref()
                            .is_some_and(|state| state.preparation_finished())
                        {
                            break;
                        }
                        assert!(
                            std::time::Instant::now() < deadline,
                            "MSP review did not prepare"
                        );
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                    harness.run();
                    // Native MSP now owns an immutable preparation/review route.
                    // Its actual V1/V2 saves are exercised in msp_export::tests.
                    assert_eq!(harness.state().confirm, Some(Confirm::MspExport));
                    assert!(harness.state().msp_export.is_some());
                    harness.get_by_label(&translate(language, "Cancel")).click();
                    harness.run();
                    assert!(harness.state().confirm.is_none());
                    assert!(!path.with_extension(format.extension()).exists());
                    assert!(!harness.state().editor.is_dirty());
                    continue;
                }
                harness.run();
                let Some(Confirm::Export {
                    path: selected,
                    format: selected_format,
                    jpeg_quality,
                    ..
                }) = &harness.state().confirm
                else {
                    panic!("{} {format:?}: missing loss review", language.id());
                };
                assert_eq!(selected, &path.with_extension(format.extension()));
                assert_eq!(
                    Some(*selected_format),
                    ImageFormat::from_extension(&path.with_extension(format.extension()))
                );
                assert_eq!(*jpeg_quality, 43);
                assert!(!selected.exists());
                harness.get_by_label(&translate(language, "Cancel")).click();
                harness.run();
                assert!(harness.state().confirm.is_none());
                assert!(!path.with_extension(format.extension()).exists());
                assert!(!harness.state().editor.is_dirty());
            }
        }
    }

    #[test]
    fn native_export_completion_preserves_the_project_destination_and_options() {
        let mut app = App::new(None);
        app.path_text = "my-project.pcnd".into();
        let path = std::env::temp_dir().join(format!("paintcraft-export-{}.jpg", Uuid::new_v4()));
        app.export_dialog_quality = Some(31);
        app.complete_dialog(
            &egui::Context::default(),
            paintcraft_platform::dialogs::Completion {
                kind: Kind::Export,
                path: Some(path.clone()),
            },
        );
        assert_eq!(app.path_text, "my-project.pcnd");
        assert!(app.bound.is_none());
        assert!(app.editor.path().is_none());
        assert!(matches!(
            app.confirm,
            Some(Confirm::Export {
                jpeg_quality: 31,
                format: ImageFormat::Jpeg,
                ..
            })
        ));
        assert!(app.export_dialog_quality.is_none());
        assert!(!path.exists());
    }
}

#[cfg(test)]
#[path = "export_options/advanced_metadata_tests.rs"]
mod advanced_metadata_tests;

#[cfg(test)]
#[path = "export_options/filter_bake_tests.rs"]
mod filter_bake_tests;
