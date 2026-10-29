use super::*;
use crate::{WebApp, editor::Editor};
use eframe::egui;

fn loaded() -> Loaded {
    Loaded {
        raster_preview: true,
        converted_copy: false,
        document: Editor::new(2, 1).unwrap().session.document().clone(),
        name: "new.png".into(),
        losses: vec!["depth-16-reduced-to-8".into()],
    }
}
fn stage(app: &mut WebApp, ctx: &egui::Context) {
    app.import_owner = Some(Owner::capture(app));
    app.accept_loaded(loaded(), ctx);
}

#[test]
fn import_consent_keeps_the_old_session_until_accept_and_cancel_releases_source() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let before = app.editor.session.snapshot();
    let ticket = app.editor.session.ticket();
    stage(&mut app, &ctx);
    assert!(app.pending_import.is_some());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    assert_eq!(app.editor.session.ticket(), ticket);
    app.resolve_import(&ctx, false);
    assert!(app.pending_import.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
    stage(&mut app, &ctx);
    let ctx = super::raster_review::tests::authorize(&mut app);
    app.resolve_import(&ctx, true);
    assert_eq!(app.editor.session.document().canvas, (2, 1));
    let accepted = app.editor.session.snapshot();
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&accepted, &app.editor.session.snapshot()));
}

#[test]
fn source_profile_drafts_and_session_replacement_invalidate_pending_or_late_completion() {
    let ctx = egui::Context::default();
    for case in 0..5 {
        let mut app = WebApp::new(&ctx);
        stage(&mut app, &ctx);
        match case {
            0 => app.variant = ClassicVariant::V7,
            1 => {
                let doc = app.editor.session.document().clone();
                app.editor = Editor::from_document(doc).unwrap();
            }
            2 => app.text.preedit = "composition".into(),
            3 => {
                app.editor.tools.tool = paintcraft_tools::Tool::Pencil;
                app.editor
                    .tools
                    .pointer_down(app.editor.target.unwrap(), (1, 1));
            }
            _ => {
                app.editor = Editor::new(3, 3).unwrap();
            }
        }
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        app.resolve_import(&ctx, true);
        assert!(
            Arc::ptr_eq(&before, &app.editor.session.snapshot()),
            "case{case}"
        );
        assert_eq!(app.editor.session.ticket(), ticket);
        assert!(app.pending_import.is_none());
    }
    let mut app = WebApp::new(&ctx);
    app.import_owner = Some(Owner::capture(&app));
    app.editor = Editor::new(3, 3).unwrap();
    let before = app.editor.session.snapshot();
    app.accept_loaded(loaded(), &ctx);
    assert!(app.pending_import.is_none());
    assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
}

#[test]
fn real_gif_and_tiff_require_translated_browser_consent_in_every_skin() {
    use egui_kittest::{Harness, kittest::Queryable};
    use paintcraft_formats::{
        image::{self, ImageFormat},
        registry::Frontend,
    };
    use paintcraft_ui::i18n::{Language, set_language, translate};
    for (format, extension) in [(ImageFormat::Gif, "gif"), (ImageFormat::Tiff, "tiff")] {
        let bytes = image::encode(format, 2, 1, &[255, 0, 0, 255, 0, 255, 0, 255], 90).unwrap();
        for skin in [ClassicVariant::V7, ClassicVariant::V10, ClassicVariant::V11] {
            let frontend = match skin {
                ClassicVariant::V7 => Frontend::ClassicV7,
                ClassicVariant::V10 => Frontend::ClassicV10,
                ClassicVariant::V11 => Frontend::ClassicV11,
            };
            for language in [Language::English, Language::Japanese] {
                let source = bytes.clone();
                let name = format!("image.{extension}");
                let mut h = Harness::builder()
                    .with_size(egui::Vec2::new(1360.0, 900.0))
                    .build_eframe(move |cc| {
                        let mut app = WebApp::new(&cc.egui_ctx);
                        app.variant = skin;
                        set_language(&cc.egui_ctx, language);
                        app.import_owner = Some(Owner::capture(&app));
                        app.accept_loaded(
                            crate::files::decode_for_profile(&source, name, frontend).unwrap(),
                            &cc.egui_ctx,
                        );
                        app
                    });
                h.run();
                let before = h.state().editor.session.snapshot();
                let ticket = h.state().editor.session.ticket();
                assert!(h.state().pending_import.is_some());
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Cancel"))
                    .click();
                h.run();
                assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
                assert_eq!(h.state().editor.session.ticket(), ticket);
                let ctx = h.ctx.clone();
                let app = h.state_mut();
                app.import_owner = Some(Owner::capture(app));
                app.accept_loaded(
                    crate::files::decode_for_profile(
                        &bytes,
                        format!("image.{extension}"),
                        frontend,
                    )
                    .unwrap(),
                    &ctx,
                );
                h.run();
                h.get_by_label(&translate(language, "Accept import limitations"))
                    .click();
                h.run();
                h.get_by_role_and_label(egui::Role::Button, &translate(language, "Open image"))
                    .click();
                h.run();
                assert!(h.state().pending_import.is_none());
                assert_eq!(h.state().editor.session.document().canvas, (2, 1));
            }
        }
    }
}

#[test]
fn confirmation_rejects_a_frontend_switch_even_when_skin_and_document_match() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let source = app.editor.session.snapshot();
    stage(&mut app, &ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.resolve_import(&ctx, true);
    assert!(Arc::ptr_eq(&source, &app.editor.session.snapshot()));
    assert!(app.pending_import.is_none());
}
