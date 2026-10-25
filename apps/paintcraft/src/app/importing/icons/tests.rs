use super::*;

fn fixture() -> Vec<u8> {
    include_bytes!(
        "../../../../../../crates/services/paintcraft-formats/tests/icon-fixtures/mixed.cur"
    )
    .to_vec()
}
fn stage(app: &mut App) {
    app.settings.experience = Experience::Studio;
    app.stage_icon_import(fixture(), PathBuf::from("mixed.cur"))
        .unwrap();
}

#[test]
fn icon_entry_requires_selection_and_separate_consent_without_binding_source() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    stage(&mut app);
    let before = app.editor.document_snapshot();
    let ticket = app.editor.render_key();
    let Candidate::Entries(entries) = &app.pending_import.as_ref().unwrap().candidate else {
        panic!()
    };
    assert_eq!(entries.selected, None);
    assert_eq!(entries.entries.len(), 6);
    assert_eq!(entries.entries[1].hotspot, Some((1, 2)));
    app.resolve_import(&ctx, true);
    assert!(matches!(
        app.pending_import.as_ref().unwrap().candidate,
        Candidate::Entries(_)
    ));
    app.choose_icon_entry(&ctx, 8);
    assert!(matches!(
        app.pending_import.as_ref().unwrap().candidate,
        Candidate::Entries(_)
    ));
    app.choose_icon_entry(&ctx, 1);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(ticket, app.editor.render_key());
    app.resolve_import(&ctx, false);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    stage(&mut app);
    app.choose_icon_entry(&ctx, 1);
    app.resolve_import(&ctx, true);
    assert_eq!(app.editor.document().canvas, (19, 19));
    assert!(app.editor.is_dirty());
    assert!(app.bound.is_none());
    assert!(app.editor.path().is_none());
    assert!(app.recent.entries.is_empty());
    let expected = icons::decode_entry(&fixture(), 1, 65536).unwrap();
    let layer = &app.editor.document().layers[&app.editor.document().roots[0]];
    for (index, rgba) in expected.rgba.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer.raster.get((index % 19) as i64, (index / 19) as i64).0,
            *rgba
        );
    }
}

#[test]
fn stale_document_and_classic_profile_refuse_icon_publication() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    assert!(
        app.stage_icon_import(fixture(), PathBuf::from("mixed.cur"))
            .is_err()
    );
    stage(&mut app);
    app.editor = Editor::with_document(app.editor.document().clone(), None);
    let before = app.editor.document_snapshot();
    app.choose_icon_entry(&ctx, 1);
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert!(app.pending_import.is_none());
}

#[test]
fn native_icon_controls_require_explicit_entry_then_separate_consent() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, format_message, translate};
    for language in [Language::English, Language::Japanese] {
        let mut app = App::new(None);
        stage(&mut app);
        app.settings.language = language;
        let before = app.editor.document_snapshot();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
        h.run();
        assert!(
            h.get_by_label(&translate(language, "Import selected entry"))
                .accesskit_node()
                .is_disabled()
        );
        let label = format_message(
            language,
            "Entry { $number }: { $width } × { $height } px, { $encoding }, { $depth } bits",
            &[
                ("number", "2".into()),
                ("width", "19".into()),
                ("height", "19".into()),
                ("encoding", "DIB".into()),
                ("depth", "4".into()),
            ],
        );
        h.get_by_label(&label).click();
        h.run();
        h.get_by_role_and_label(
            egui::Role::Button,
            &translate(language, "Import selected entry"),
        )
        .click();
        h.run();
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        assert_eq!(h.state().editor.document().canvas, (19, 19));
    }
}
