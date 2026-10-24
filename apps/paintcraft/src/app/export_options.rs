//! Staged image encoder choices, followed by a native destination picker.
use super::*;
use paintcraft_ui::{actions::ImageExport, t};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Draft {
    owner: Uuid,
    format: ImageExport,
    jpeg_quality: u8,
}

impl App {
    pub(super) fn begin_export_options(&mut self, format: ImageExport) {
        if self.document_count() == 0
            || !self.editor.shape_file_ready()
            || self.dialogs.as_ref().is_some_and(NativeDialogs::pending)
        {
            return;
        }
        self.editor.cancel_gesture();
        self.confirm = Some(Confirm::ExportOptions(Draft {
            owner: self.active_tab_id(),
            format,
            jpeg_quality: 90,
        }));
    }

    pub(super) fn export_options_dialog(&mut self, ctx: &egui::Context, mut draft: Draft) {
        if draft.owner != self.active_tab_id() {
            self.confirm = None;
            return;
        }
        let response = egui::Modal::new(egui::Id::unique("image-export-options")).show(ctx, |ui| {
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
                        for format in ImageExport::ALL {
                            ui.selectable_value(&mut draft.format, format, t!(ctx, format.label()));
                        }
                    })
                    .response
                    .labelled_by(label.id);
            });
            ui.label(t!(ctx, draft.format.description()));
            if draft.format == ImageExport::Jpeg {
                ui.add_space(8.0);
                ui.add(
                    egui::Slider::new(&mut draft.jpeg_quality, 1..=100)
                        .text(t!(ctx, "JPEG quality")),
                );
                ui.label(t!(
                    ctx,
                    "Higher quality creates a larger file. Transparent pixels are placed on white."
                ));
            }
            let canvas = self.editor.document().canvas;
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
                let accept = ui.button(t!(ctx, "Choose destination…")).clicked();
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
        } else if accept {
            self.confirm = None;
            let suggested = self
                .bound
                .as_deref()
                .unwrap_or_else(|| Path::new(&self.path_text))
                .with_extension(draft.format.extension());
            if self.dialogs.is_some() {
                self.export_dialog_quality = Some(draft.jpeg_quality);
                self.request_dialog_suggested(ctx, Kind::Export, Some(suggested));
            } else {
                self.export_selected_quality(suggested, draft.jpeg_quality);
            }
        } else {
            self.confirm = Some(Confirm::ExportOptions(draft));
        }
    }
}

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
                format: ImageExport::Jpeg,
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
                let path = std::env::temp_dir()
                    .join(format!("paintcraft-export-dialog-{}.pcnd", Uuid::new_v4()));
                app.path_text = path.to_string_lossy().into_owned();
                app.begin_export_options(format);
                if let Some(Confirm::ExportOptions(draft)) = &mut app.confirm {
                    draft.jpeg_quality = 43;
                }
                let mut harness = Harness::builder()
                    .with_size(egui::vec2(800.0, 600.0))
                    .build_ui_state(
                        move |ui, app: &mut App| {
                            set_language(ui.ctx(), language);
                            app.confirm_dialog(ui);
                        },
                        app,
                    );
                harness.run();
                harness
                    .get_by_label(&translate(language, "Choose destination…"))
                    .click();
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
                assert_eq!(selected_format.name(), format.name());
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
