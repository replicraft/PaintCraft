use super::*;

fn fixture() -> Vec<u8> {
    include_bytes!(
        "../../../../../../crates/services/paintcraft-formats/tests/icon-fixtures/mixed.cur"
    )
    .to_vec()
}
fn stage(app: &mut WebApp, ctx: &egui::Context) {
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.import_owner = Some(Owner::capture(app));
    app.accept_pages(
        Source::new(fixture(), "mixed.cur".into(), Frontend::Studio).unwrap(),
        ctx,
    );
}

#[test]
fn icon_entry_cancel_consent_and_stale_owner_preserve_original() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    stage(&mut app, &ctx);
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    let pending = app.pending_pages.as_ref().unwrap();
    assert_eq!(pending.selected, None);
    assert_eq!(
        pending.source.entries.as_ref().unwrap()[1].hotspot,
        Some((1, 2))
    );
    app.choose_page_import(&ctx, 8);
    assert!(app.pending_pages.as_ref().unwrap().error.is_some());
    app.choose_page_import(&ctx, 1);
    assert!(app.pending_import.is_some());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(ticket, app.editor.session.ticket());
    app.resolve_import(&ctx, false);
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    stage(&mut app, &ctx);
    app.editor = crate::editor::Editor::new(3, 3).unwrap();
    let changed = app.editor.session.snapshot();
    app.choose_page_import(&ctx, 1);
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&changed, &app.editor.session.snapshot()));
    assert!(app.pending_pages.is_none());
    stage(&mut app, &ctx);
    app.choose_page_import(&ctx, 1);
    app.resolve_import(&ctx, true);
    let doc = app.editor.session.document();
    assert_eq!(doc.canvas, (19, 19));
    assert!(app.editor.session.is_dirty());
    let expected = paintcraft_formats::icons::decode_entry(&fixture(), 1, 65536).unwrap();
    let layer = &doc.layers[&doc.roots[0]];
    for (index, rgba) in expected.rgba.as_chunks::<4>().0.iter().enumerate() {
        assert_eq!(
            layer.raster.get((index % 19) as i64, (index / 19) as i64).0,
            *rgba
        );
    }
    for profile in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
    ] {
        assert!(Source::new(fixture(), "mixed.cur".into(), profile).is_err());
    }
    assert!(Source::new(fixture(), "disguised.ico".into(), Frontend::Studio).is_err());
}

#[test]
fn browser_icon_controls_show_hotspots_and_require_explicit_entry_then_consent() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use paintcraft_ui::i18n::{Language, format_message, translate};
    for language in [Language::English, Language::Japanese] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 900.0))
            .build_eframe(|cc| {
                let mut app = WebApp::new(&cc.egui_ctx);
                paintcraft_ui::i18n::set_language(&cc.egui_ctx, language);
                stage(&mut app, &cc.egui_ctx);
                app
            });
        h.run();
        let before = h.state().editor.session.snapshot();
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
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert!(h.state().pending_import.is_some());
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
            .click();
        h.run();
        assert_eq!(h.state().editor.session.document().canvas, (19, 19));
    }
}
