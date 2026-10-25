use super::*;

fn image() -> image::Decoded {
    image::Decoded {
        format: ImageFormat::Png,
        width: 1,
        height: 1,
        rgba: vec![13, 24, 35, 0],
        losses: vec!["depth-16-reduced-to-8".into()],
    }
}
fn stage(app: &mut App) {
    app.stage_import(image(), PathBuf::from("capture.png"), "ab".repeat(32))
        .unwrap();
}

#[test]
fn consent_is_required_before_publication_binding_recent_or_history_changes() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    stage(&mut app);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.editor.history_view().ticket, ticket);
    assert!(app.recent.entries.is_empty());
    assert!(app.bound.is_none());
    app.resolve_import(&ctx, false);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.pending_import.is_none());
    assert!(app.confirm.is_none());
    stage(&mut app);
    app.resolve_import(&ctx, true);
    assert_eq!(app.editor.document().canvas, (1, 1));
    let layer = &app.editor.document().layers[&app.editor.document().roots[0]];
    assert_eq!(layer.raster.get(0, 0), Rgba([13, 24, 35, 0]));
    assert!(app.editor.path().is_none());
    assert!(app.bound.is_none());
    assert_eq!(app.recent.entries.len(), 1);
    let accepted = app.editor.document_snapshot();
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&accepted, &app.editor.document_snapshot()));
}

#[test]
fn stale_owner_source_profile_pending_edits_or_drafts_cannot_be_discarded_by_accept() {
    let ctx = egui::Context::default();
    for case in 0..6 {
        let mut app = App::new(None);
        stage(&mut app);
        match case {
            0 => {
                app.add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
                    .unwrap();
            }
            1 => {
                let same = app.editor.document().clone();
                app.editor = Editor::with_document(same, None);
            }
            2 => {
                app.settings.classic_variant = Some(ClassicVariant::V7);
            }
            3 => {
                app.editor.request(
                    481,
                    Action::ClearCanvas {
                        color: Rgba::TRANSPARENT,
                    },
                );
            }
            4 => {
                app.editor.pointer_down((1, 1));
            }
            _ => {
                app.editor.request(
                    482,
                    Action::ClearCanvas {
                        color: Rgba([7, 8, 9, 255]),
                    },
                );
                app.editor.run_pending();
            }
        }
        let before = app.editor.document_snapshot();
        let ticket = app.editor.history_view().ticket;
        let pending = app.editor.has_pending_edits();
        let draft = app.editor.gesture_in_progress();
        app.resolve_import(&ctx, true);
        assert!(
            Arc::ptr_eq(&before, &app.editor.document_snapshot()),
            "case{case}"
        );
        assert_eq!(app.editor.history_view().ticket, ticket);
        assert_eq!(app.editor.has_pending_edits(), pending);
        assert_eq!(app.editor.gesture_in_progress(), draft);
        assert!(app.pending_import.is_none());
        assert!(app.recent.entries.is_empty());
    }
}

#[test]
fn unknown_loss_cannot_be_silently_accepted() {
    let mut app = App::new(None);
    let mut source = image();
    source.losses.push("unknown-loss".into());
    assert!(
        app.stage_import(source, PathBuf::from("unknown.png"), "ab".repeat(32))
            .is_err()
    );
    assert!(app.pending_import.is_none());
    assert!(app.confirm.is_none());
}

#[test]
fn real_gif_and_tiff_consent_controls_work_across_skins_and_languages() {
    use egui_kittest::{Harness, kittest::Queryable};
    use paintcraft_ui::i18n::{Language, translate};
    let dir = std::env::temp_dir().join(format!("paintcraft-import-ui-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    for (format, extension) in [(ImageFormat::Gif, "gif"), (ImageFormat::Tiff, "tiff")] {
        let bytes = image::encode(format, 2, 1, &[255, 0, 0, 255, 0, 255, 0, 255], 90).unwrap();
        let path = dir.join(format!("image.{extension}"));
        std::fs::write(&path, &bytes).unwrap();
        let expected = image::decode(&bytes).unwrap().rgba;
        for (experience, skin) in [
            (Experience::Classic, ClassicVariant::V7),
            (Experience::Classic, ClassicVariant::V10),
            (Experience::Classic, ClassicVariant::V11),
            (Experience::Studio, ClassicVariant::V11),
        ] {
            for language in [Language::English, Language::Japanese] {
                let mut app = App::new(None);
                app.settings.experience = experience;
                app.settings.classic_variant = Some(skin);
                app.settings.language = language;
                let before = app.editor.document_snapshot();
                let ticket = app.editor.history_view().ticket;
                app.open_path(&path).unwrap();
                assert!(app.pending_import.is_some());
                let mut h = Harness::builder()
                    .with_size(Vec2::new(1360.0, 900.0))
                    .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
                h.run();
                assert!(
                    h.query_by_label(&translate(language, "Open with these import limitations?"))
                        .is_some()
                );
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                    .click();
                h.run();
                assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
                assert_eq!(h.state().editor.history_view().ticket, ticket);
                assert!(h.state().recent.entries.is_empty());
                h.state_mut().open_path(&path).unwrap();
                h.run();
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .click();
                h.run();
                assert!(h.state().pending_import.is_none());
                assert_eq!(h.state().editor.document().canvas, (2, 1));
                let document = h.state().editor.document();
                let layer = &document.layers[&document.roots[0]];
                assert_eq!(layer.raster.get(0, 0).0.as_slice(), &expected[..4]);
                assert_eq!(layer.raster.get(1, 0).0.as_slice(), &expected[4..8]);
                assert!(h.state().bound.is_none());
                assert_eq!(h.state().recent.entries.len(), 1);
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn picker_inventory_uses_qualified_read_aliases_but_not_unqualified_writers() {
    for skin in [ClassicVariant::V7, ClassicVariant::V10, ClassicVariant::V11] {
        let mut app = App::new(None);
        app.settings.classic_variant = Some(skin);
        let open = app.format_filters(Kind::Open);
        let aliases = &open[0].extensions;
        for extension in ["pcnd", "png", "gif", "tif", "tiff"] {
            assert!(aliases.contains(&extension.into()));
        }
        for extension in ["svg", "pdn", "psd", "paint", "webp", "heic", "tga", "dds"] {
            assert!(!aliases.contains(&extension.into()));
        }
        let export = app.format_filters(Kind::Export);
        assert!(
            export
                .iter()
                .all(|filter| filter.extensions.iter().all(|ext| !matches!(
                    ext.as_str(),
                    "gif" | "tif" | "tiff" | "heic" | "heif" | "tga" | "webp"
                )))
        );
    }
}

#[test]
fn late_native_open_completion_cannot_replace_a_fresh_session_with_the_same_ids() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.dialog_owner = Some(app.active_tab_id());
    app.dialog_frontend = Some(app.format_frontend());
    app.dialog_source = Some((app.editor.document_snapshot(), app.editor.render_key()));
    let document = app.editor.document().clone();
    app.editor = Editor::with_document(document, None);
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    app.complete_dialog(
        &ctx,
        paintcraft_platform::dialogs::Completion {
            kind: Kind::Open,
            path: Some(PathBuf::from("not-acquired.png")),
        },
    );
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(app.editor.history_view().ticket, ticket);
    assert!(app.pending_import.is_none());
    assert!(app.recent.entries.is_empty());
}
