use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, set_language, translate};

#[test]
fn jpeg_matte_control_and_loss_review_keep_the_captured_color_and_current_project() {
    for language in Language::ALL {
        let rgba: Vec<u8> = (0..64).flat_map(|_| [17, 92, 231, 128]).collect();
        let doc = paintcraft_document::import_image(8, 8, &rgba, "native-jpeg-matte").unwrap();
        let mut app = App::new(None);
        app.editor = Editor::with_document(doc, None);
        let source = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let dirty = app.editor.is_dirty();
        let path = std::env::temp_dir().join(format!("paintcraft-matte-{}.jpg", Uuid::new_v4()));
        app.path_text = path.to_string_lossy().into_owned();
        app.begin_export_options(ImageExport::Jpeg);
        let Some(Confirm::ExportOptions(draft)) = &mut app.confirm else {
            panic!("export options")
        };
        assert_eq!(draft.jpeg_matte, [255; 3]);
        draft.jpeg_matte = [32, 160, 224];
        let mut h = Harness::builder()
            .with_size(egui::vec2(900., 800.))
            .build_ui_state(
                move |ui, app: &mut App| {
                    set_language(ui.ctx(), language);
                    app.confirm_dialog(ui);
                },
                app,
            );
        let tr = |source| translate(language, source);
        h.run();
        h.get_by_label(&tr("JPEG background color"));
        h.get_by_label(&tr("Transparent pixels use the selected background color."));
        h.get_by_label(&tr("Choose destination…")).click();
        h.run();
        let Some(Confirm::Export {
            jpeg_matte, losses, ..
        }) = &h.state().confirm
        else {
            panic!("loss review")
        };
        assert_eq!(
            *jpeg_matte,
            Some(paintcraft_formats::image::JpegMatte::new([32, 160, 224]))
        );
        assert!(losses.iter().any(|v| v == "lossy-jpeg"));
        assert!(losses.iter().any(|v| v == "alpha-discarded"));
        h.get_by_label(&tr("Export now")).click();
        h.run();
        let bytes = std::fs::read(&path).unwrap();
        let decoded = paintcraft_formats::image::decode(&bytes).unwrap();
        assert!(decoded.rgba.as_chunks::<4>().0.iter().all(|pixel| {
            pixel[..3]
                .iter()
                .zip([24u8, 126, 228])
                .all(|(a, b)| a.abs_diff(b) <= 2)
        }));
        assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
        assert_eq!(ticket, h.state().editor.history_view().ticket);
        assert_eq!(dirty, h.state().editor.is_dirty());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn cancelling_matte_options_and_native_picker_drops_color_without_exporting() {
    let mut app = App::new(None);
    app.begin_export_options(ImageExport::Jpeg);
    let Some(Confirm::ExportOptions(draft)) = &mut app.confirm else {
        panic!("options")
    };
    draft.jpeg_matte = [12, 34, 56];
    let mut h = Harness::builder()
        .with_size(egui::vec2(900., 800.))
        .build_ui_state(|ui, app: &mut App| app.confirm_dialog(ui), app);
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().confirm.is_none());
    assert!(h.state().export_dialog_matte.is_none());
    h.state_mut().export_dialog_matte =
        Some(paintcraft_formats::image::JpegMatte::new([12, 34, 56]));
    h.state_mut().complete_dialog(
        &egui::Context::default(),
        paintcraft_platform::dialogs::Completion {
            kind: paintcraft_platform::dialogs::Kind::Export,
            path: None,
        },
    );
    assert!(h.state().export_dialog_matte.is_none());
}
