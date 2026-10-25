use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{Language, translate};

#[test]
fn photoshop_import_uses_translated_consent_and_keeps_original_and_layer_identity() {
    let dir = std::env::temp_dir().join(format!("paintcraft-psd-ui-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    for (extension, bytes) in [
        (
            "psd",
            include_bytes!(
                "../../../../../crates/services/paintcraft-formats/tests/psd-fixtures/layers-3.psd"
            )
            .as_slice(),
        ),
        (
            "psb",
            include_bytes!(
                "../../../../../crates/services/paintcraft-formats/tests/psd-fixtures/layers-1.psb"
            )
            .as_slice(),
        ),
    ] {
        let path = dir.join(format!("source.{extension}"));
        std::fs::write(&path, bytes).unwrap();
        for language in [Language::English, Language::Japanese] {
            let mut app = App::new(None);
            app.settings.experience = Experience::Studio;
            app.settings.language = language;
            assert!(
                !app.format_filters(Kind::Open)[0]
                    .extensions
                    .contains(&extension.into())
            );
            assert!(
                app.format_filters(Kind::Import)[0]
                    .extensions
                    .contains(&extension.into())
            );
            let before = app.editor.document_snapshot();
            let ticket = app.editor.history_view().ticket;
            assert!(app.open_path(&path).is_err());
            app.import_selected(path.clone());
            assert!(app.pending_import.is_some(), "{}", app.editor.status());
            let mut h = Harness::builder()
                .with_size(Vec2::new(1360.0, 900.0))
                .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
            h.run();
            let description = import_loss_description("psd-metadata-ignored").unwrap();
            assert!(
                h.query_by_label(&translate(language, description))
                    .is_some()
            );
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                .click();
            h.run();
            assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
            assert_eq!(h.state().editor.history_view().ticket, ticket);
            assert!(h.state().recent.entries.is_empty());
            h.state_mut().import_selected(path.clone());
            h.run();
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                .click();
            h.run();
            assert!(h.state().pending_import.is_none());
            let doc = h.state().editor.document();
            assert_eq!((doc.canvas, doc.roots.len()), ((5, 3), 2));
            let first = &doc.layers[&doc.roots[0]];
            assert_eq!(
                (
                    first.name.as_str(),
                    first.origin,
                    first.visible,
                    first.opacity
                ),
                ("日本語 duplicate", (-1, 1), false, 127)
            );
            assert_eq!(first.raster.get(0, 0).0, [9, 38, 67, 96]);
            assert_ne!(doc.roots[0], doc.roots[1]);
            assert!(h.state().editor.is_dirty());
            assert!(h.state().bound.is_none());
            assert!(h.state().editor.path().is_none());
            assert_eq!(h.state().imported_source.as_deref(), Some(path.as_path()));
            assert!(
                paintcraft_formats::pcnd::read(
                    &paintcraft_formats::pcnd::write_compact(doc).unwrap()
                )
                .unwrap()
                .same_semantics(doc)
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn stale_photoshop_import_cannot_replace_the_current_document() {
    let ctx = egui::Context::default();
    let bytes = include_bytes!(
        "../../../../../crates/services/paintcraft-formats/tests/psd-fixtures/layers-0.psd"
    );
    for stale in [false, true] {
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        let converted =
            paintcraft_formats::project_import::decode_psd(bytes, 100, &|| Ok(())).unwrap();
        app.stage_project_import(converted, "source.psd".into())
            .unwrap();
        if stale {
            app.editor.request(
                993,
                Action::ClearCanvas {
                    color: Rgba([255; 4]),
                },
            );
        } else {
            app.settings.experience = Experience::Classic;
        }
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        app.resolve_import(&ctx, true);
        assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert!(app.recent.entries.is_empty());
    }
}
