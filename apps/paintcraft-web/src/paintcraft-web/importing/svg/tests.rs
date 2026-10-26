use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use paintcraft_ui::i18n::{self, Language, translate};

const SOURCE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16"><rect width="32" height="16" fill="#e04020"/></svg>"##;
fn stage(app: &mut WebApp, ctx: &egui::Context) {
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.import_owner = Some(Owner::capture(app));
    app.accept_svg(
        Source::new(SOURCE.to_vec(), "source.svg".into(), Frontend::Studio).unwrap(),
        ctx,
    );
    let state = &mut app.pending_svg.as_mut().unwrap().state;
    state.config.width = 64;
    state.config.height = 32;
}
#[test]
fn browser_svg_controls_preview_and_consent_keep_native_copy_unsaved() {
    for language in [Language::English, Language::Japanese] {
        let ctx = egui::Context::default();
        let mut app = WebApp::new(&ctx);
        stage(&mut app, &ctx);
        let before = app.editor.session.snapshot();
        assert!(app.blocked());
        let mut h = Harness::builder()
            .with_max_steps(16)
            .with_size(egui::vec2(1360.0, 900.0))
            .build_ui_state(
                move |ui, app: &mut WebApp| {
                    i18n::set_language(ui.ctx(), language);
                    app.svg_import_dialog(ui.ctx());
                },
                app,
            );
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Update preview"))
            .click();
        h.run();
        assert!(
            h.query_by_label(&translate(language, "Import preview"))
                .is_some()
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        h.get_by_label(&translate(language, "I accept these import changes."))
            .click();
        h.run();
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Import image"))
            .click();
        h.run();
        assert!(h.state().pending_svg.is_none());
        let document = h.state().editor.session.document();
        assert_eq!(document.canvas, (64, 32));
        assert_eq!(
            document.layers[&document.roots[0]].raster.get(50, 20).0,
            [224, 64, 32, 255]
        );
        assert!(h.state().editor.session.is_dirty());
        assert!(
            paintcraft_formats::pcnd::read(
                &paintcraft_formats::pcnd::write_compact(document).unwrap()
            )
            .unwrap()
            .same_semantics(document)
        );
    }
}
#[test]
fn malformed_classic_cancelled_and_stale_svg_imports_cannot_publish() {
    assert!(Source::new(SOURCE.to_vec(), "source.svg".into(), Frontend::ClassicV11).is_err());
    assert!(Source::new(SOURCE.to_vec(), "source.svgz".into(), Frontend::Studio).is_err());
    assert!(Source::new(b"<svg>".to_vec(), "source.svg".into(), Frontend::Studio).is_err());
    let ctx = egui::Context::default();
    for case in 0..3 {
        let mut app = WebApp::new(&ctx);
        stage(&mut app, &ctx);
        let before = app.editor.session.snapshot();
        let ticket = app.editor.session.ticket();
        match case {
            0 => app.pending_svg = None,
            1 => app.frontend = paintcraft_ui::actions::Frontend::Classic,
            _ => app.text.preedit = "composition".into(),
        }
        app.svg_import_dialog(&ctx);
        assert!(app.pending_svg.is_none());
        assert!(Arc::ptr_eq(&before, &app.editor.session.snapshot()));
        assert_eq!(ticket, app.editor.session.ticket());
    }
}

#[test]
fn narrow_svg_dialog_keeps_footer_visible_and_consent_reachable() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    stage(&mut app, &ctx);
    let before = app.editor.session.snapshot();
    let mut h = Harness::builder()
        .with_max_steps(20)
        .with_size(egui::vec2(390.0, 640.0))
        .build_ui_state(|ui, app: &mut WebApp| app.svg_import_dialog(ui.ctx()), app);
    h.run();
    h.get_by_role_and_label(egui::Role::Button, "Update preview")
        .click();
    h.run();
    for label in ["Cancel", "Import image"] {
        let rect = h.get_by_role_and_label(egui::Role::Button, label).rect();
        assert!(
            h.ctx.content_rect().contains_rect(rect),
            "{label}: {rect:?}"
        );
    }
    h.get_by_label("I accept these import changes.")
        .scroll_to_me();
    h.run();
    let consent = h.get_by_label("I accept these import changes.").rect();
    assert!(h.ctx.content_rect().contains_rect(consent));
    h.get_by_label("I accept these import changes.").click();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().pending_svg.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
}
