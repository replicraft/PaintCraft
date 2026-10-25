use super::*;

fn fixture() -> Vec<u8> {
    let mut bytes = vec![0; 4100];
    bytes[..4].copy_from_slice(&987_654_321u32.to_le_bytes());
    for (index, rgb) in [[10, 20, 30], [200, 210, 220]].into_iter().enumerate() {
        let offset = bytes.len() as u32;
        bytes[4 + index * 4..8 + index * 4].copy_from_slice(&offset.to_le_bytes());
        let mut pcx = vec![0; 128];
        pcx[..4].copy_from_slice(&[10, 5, 1, 8]);
        pcx[65] = 3;
        pcx[66..68].copy_from_slice(&2u16.to_le_bytes());
        for value in rgb {
            if value >= 192 {
                pcx.extend([0xc1, value]);
            } else {
                pcx.push(value);
            }
            pcx.push(0);
        }
        bytes.extend(pcx);
    }
    bytes
}
fn stage(app: &mut App) {
    app.settings.experience = Experience::Studio;
    app.stage_dcx_import(fixture(), PathBuf::from("pages.dcx"))
        .unwrap();
}
#[test]
fn explicit_selection_and_loss_consent_never_implicitly_publish_page_zero() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    stage(&mut app);
    let before = app.editor.document_snapshot();
    let ticket = app.editor.render_key();
    let Candidate::Pages(pages) = &app.pending_import.as_ref().unwrap().candidate else {
        panic!()
    };
    assert_eq!(pages.selected, None);
    assert_eq!(pages.pages.len(), 2);
    app.resolve_import(&ctx, true);
    assert!(matches!(
        app.pending_import.as_ref().unwrap().candidate,
        Candidate::Pages(_)
    ));
    app.choose_dcx_page(&ctx, 9);
    assert!(matches!(
        app.pending_import.as_ref().unwrap().candidate,
        Candidate::Pages(_)
    ));
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.choose_dcx_page(&ctx, 1);
    assert!(matches!(
        app.pending_import.as_ref().unwrap().candidate,
        Candidate::Raster(_)
    ));
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(ticket, app.editor.render_key());
    app.resolve_import(&ctx, false);
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    stage(&mut app);
    app.choose_dcx_page(&ctx, 1);
    app.resolve_import(&ctx, true);
    let doc = app.editor.document();
    assert_eq!(doc.canvas, (1, 1));
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(0, 0).0,
        [200, 210, 220, 255]
    );
    assert!(app.bound.is_none());
    assert!(app.editor.path().is_none());
    assert!(app.recent.entries.is_empty());
}
#[test]
fn stale_tab_frontend_edits_and_cancel_preserve_source() {
    let ctx = egui::Context::default();
    for case in 0..5 {
        let mut app = App::new(None);
        stage(&mut app);
        match case {
            0 => app.resolve_import(&ctx, false),
            1 => app.settings.experience = Experience::Classic,
            2 => {
                app.add_document_tab(Editor::new_document(3, 3).unwrap(), &ctx)
                    .unwrap();
            }
            3 => {
                app.editor.pointer_down((1, 1));
            }
            _ => {
                app.editor = Editor::with_document(app.editor.document().clone(), None);
            }
        }
        let before = app.editor.document_snapshot();
        let ticket = app.editor.render_key();
        app.choose_dcx_page(&ctx, 1);
        app.resolve_import(&ctx, true);
        assert!(
            Arc::ptr_eq(&before, &app.editor.document_snapshot()),
            "case {case}"
        );
        assert_eq!(ticket, app.editor.render_key());
        assert!(app.pending_import.is_none());
    }
}

#[test]
fn native_page_controls_are_explicit_translated_and_keep_loss_confirmation_separate() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, format_message, translate};
    for language in Language::ALL {
        let mut app = App::new(None);
        stage(&mut app);
        app.settings.language = language;
        let before = app.editor.document_snapshot();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
        h.run();
        assert!(
            h.get_by_label(&translate(language, "Import selected page"))
                .accesskit_node()
                .is_disabled()
        );
        let label = format_message(
            language,
            "Page { $number }: { $width } × { $height } px",
            &[
                ("number", "2".into()),
                ("width", "1".into()),
                ("height", "1".into()),
            ],
        );
        h.get_by_label(&label).click();
        h.run();
        h.get_by_role_and_label(
            egui::Role::Button,
            &translate(language, "Import selected page"),
        )
        .click();
        h.run();
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert!(matches!(
            h.state().pending_import.as_ref().unwrap().candidate,
            Candidate::Raster(_)
        ));
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        assert_eq!(h.state().editor.document().canvas, (1, 1));
    }
}
#[test]
fn dcx_is_listed_only_for_explicit_studio_import() {
    let mut app = App::new(None);
    assert!(
        !app.format_filters(Kind::Import)[0]
            .extensions
            .contains(&"dcx".into())
    );
    app.settings.experience = Experience::Studio;
    assert!(
        app.format_filters(Kind::Import)[0]
            .extensions
            .contains(&"dcx".into())
    );
    assert!(
        !app.format_filters(Kind::Open)[0]
            .extensions
            .contains(&"dcx".into())
    );
    assert!(app.open_path(Path::new("pages.dcx")).is_err());
}

#[test]
fn native_picker_import_retains_frozen_bytes_even_if_disk_source_changes() {
    let ctx = egui::Context::default();
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let dir = std::env::temp_dir().join(format!("paintcraft-dcx-import-{}", Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("pages.dcx");
    std::fs::write(&path, fixture()).unwrap();
    let before = app.editor.document_snapshot();
    app.import_selected(path.clone());
    assert!(matches!(
        app.pending_import.as_ref().unwrap().candidate,
        Candidate::Pages(_)
    ));
    std::fs::write(&path, b"source changed after picker completion").unwrap();
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    app.choose_dcx_page(&ctx, 1);
    app.resolve_import(&ctx, true);
    let doc = app.editor.document();
    assert_eq!(
        doc.layers[&doc.roots[0]].raster.get(0, 0).0,
        [200, 210, 220, 255]
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        b"source changed after picker completion"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
