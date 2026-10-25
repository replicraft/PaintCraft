use super::*;
use egui_kittest::kittest::NodeT;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, set_language, translate};

#[test]
fn actual_palette_controls_preview_refusal_and_export_keep_the_original_document() {
    for language in [Language::English, Language::Japanese] {
        let rgba: Vec<u8> = (0..400u32)
            .flat_map(|i| [i as u8, (i >> 8) as u8, i.wrapping_mul(53) as u8, 255])
            .collect();
        let doc = paintcraft_document::import_image(40, 10, &rgba, "native-pcx-ui").unwrap();
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.editor = Editor::with_document(doc, None);
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let path =
            std::env::temp_dir().join(format!("paintcraft-pcx-dialog-{}.pcx", Uuid::new_v4()));
        app.path_text = path.to_string_lossy().into_owned();
        app.begin_export_options(ImageExport::Pcx);
        let mut h = Harness::builder()
            .with_size(egui::vec2(800.0, 700.0))
            .build_ui_state(
                move |ui, app: &mut App| {
                    set_language(ui.ctx(), language);
                    app.confirm_dialog(ui);
                },
                app,
            );
        let tr = |s| translate(language, s);
        h.run();
        h.get_by_label(&tr("PCX encoding")).click();
        h.run();
        h.get_by_label(&tr("Indexed: exact colors")).click();
        h.run();
        assert!(
            h.get_by_label(&tr("Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            h.query_by_label(&tr(
                "PCX preview is unavailable. Reduce the image size or choose RGB24."
            ))
            .is_some()
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        h.get_by_label(&tr("PCX encoding")).click();
        h.run();
        h.get_by_label(&tr("Indexed: up to 256 colors")).click();
        h.run();
        assert!(h.query_by_label(&tr("Export preview")).is_some());
        assert!(
            h.query_by_label(&tr(
                "Palette conversion changes colors. Exporting accepts this color loss."
            ))
            .is_some()
        );
        assert!(
            !h.get_by_label(&tr("Choose destination…"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&tr("Choose destination…")).click();
        h.run();
        let Some(Confirm::Export {
            pcx_encoding,
            losses,
            ..
        }) = &h.state().confirm
        else {
            panic!("missing loss review");
        };
        assert_eq!(*pcx_encoding, Encoding::Indexed256);
        assert!(losses.iter().any(|loss| loss == "palette-quantized"));
        assert!(!path.exists());
        h.get_by_label(&tr("Export now")).click();
        h.run();
        assert!(
            path.exists(),
            "export status: {}",
            h.state().editor.status()
        );
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes[65], 1);
        let actual = paintcraft_formats::image::decode(&bytes).unwrap();
        let expected = paintcraft_formats::pcx::prepare_rgba(
            40,
            10,
            &rgba,
            paintcraft_formats::pcx::Mode::Quantized256,
            &|| Ok(()),
        )
        .unwrap()
        .preview(40, &|| Ok(()))
        .unwrap();
        assert_eq!(actual.rgba, expected.rgba);
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.history_view().ticket, ticket);
        assert!(h.state().editor.path().is_none());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn changed_picker_source_cannot_apply_earlier_palette_settings() {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.export_dialog_stamp = Some(app.editor.preview_stamp());
    app.export_dialog_pcx = Some(Encoding::Indexed256);
    app.editor.pointer_down((1, 1));
    app.editor.pointer_up();
    app.editor.run_pending();
    let path = std::env::temp_dir().join(format!("paintcraft-pcx-stale-{}.pcx", Uuid::new_v4()));
    app.complete_dialog(
        &egui::Context::default(),
        paintcraft_platform::dialogs::Completion {
            kind: Kind::Export,
            path: Some(path.clone()),
        },
    );
    assert!(app.confirm.is_none());
    assert!(!path.exists());
    assert!(app.export_dialog_pcx.is_none());
    assert!(app.export_dialog_stamp.is_none());
}

#[test]
fn native_picker_completion_keeps_the_selected_indexed_policy() {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::new_document(8, 8).unwrap();
    app.export_dialog_stamp = Some(app.editor.preview_stamp());
    app.export_dialog_pcx = Some(Encoding::IndexedExact);
    app.export_dialog_quality = Some(37);
    let path = std::env::temp_dir().join(format!("paintcraft-pcx-picker-{}.pcx", Uuid::new_v4()));
    app.complete_dialog(
        &egui::Context::default(),
        paintcraft_platform::dialogs::Completion {
            kind: Kind::Export,
            path: Some(path.clone()),
        },
    );
    assert!(matches!(
        app.confirm,
        Some(Confirm::Export {
            pcx_encoding: Encoding::IndexedExact,
            jpeg_quality: 37,
            ..
        })
    ));
    assert!(!path.exists());
    assert!(app.export_dialog_pcx.is_none());
    assert!(app.export_dialog_quality.is_none());
}

#[test]
fn stale_loss_confirmation_cannot_export_an_image_the_preview_did_not_show() {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.editor = Editor::new_document(8, 8).unwrap();
    let path = std::env::temp_dir().join(format!("paintcraft-pcx-confirm-{}.pcx", Uuid::new_v4()));
    app.plan_export_settings(path.clone(), Settings::default());
    app.editor.pointer_down((1, 1));
    app.editor.pointer_up();
    app.editor.run_pending();
    let mut h = Harness::builder()
        .with_size(egui::vec2(800.0, 600.0))
        .build_ui_state(|ui, app: &mut App| app.confirm_dialog(ui), app);
    h.run();
    h.get_by_label("Export now").click();
    h.run();
    assert!(!path.exists());
    assert!(h.state().confirm.is_none());
    assert_eq!(
        h.state().editor.status(),
        "The document changed; choose the image again."
    );
}
