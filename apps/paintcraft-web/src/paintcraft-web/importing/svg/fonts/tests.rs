use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{self, Language, translate};
#[path = "../../../../../../../crates/services/paintcraft-formats/tests/support/svg_fonts.rs"]
mod fixture;

fn harness(language: Language, size: egui::Vec2) -> Harness<'static, WebApp> {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.import_owner = Some(Owner::capture(&app));
    app.accept_svg(
        Source::new(fixture::TEXT.to_vec(), "text.svg".into(), Frontend::Studio).unwrap(),
        &ctx,
    );
    app.pending_svg.as_mut().unwrap().state.config.width = 128;
    app.pending_svg.as_mut().unwrap().state.config.height = 48;
    Harness::builder()
        .with_max_steps(20)
        .with_size(size)
        .build_ui_state(
            move |ui, app: &mut WebApp| {
                i18n::set_language(ui.ctx(), language);
                app.svg_import_dialog(ui.ctx());
            },
            app,
        )
}
fn upload(app: &mut WebApp, result: Result<Option<Vec<FontFile>>>) {
    let pending = app.pending_svg.as_mut().unwrap();
    pending.font_busy = true;
    *pending.font_inbox.borrow_mut() = Inbox {
        key: Some(pending.font_key()),
        result: Some(result),
    };
}
fn font() -> FontFile {
    FontFile {
        name: "PaintCF.ttf".into(),
        bytes: fixture::font(),
    }
}
fn preview(h: &mut Harness<'_, WebApp>, language: Language) {
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Update preview"))
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, &translate(language, "Update preview"))
        .click();
    h.run();
}
fn consent(h: &mut Harness<'_, WebApp>, language: Language) {
    h.get_by_label(&translate(language, "I accept these import changes."))
        .scroll_to_me();
    h.run();
    h.get_by_label(&translate(language, "I accept these import changes."))
        .click();
    h.run();
    assert!(h.state().pending_svg.as_ref().unwrap().state.accepted);
}

#[test]
fn browser_font_upload_changes_real_text_and_requires_new_preview_and_consent_in_both_languages() {
    for language in [Language::English, Language::Japanese] {
        let mut h = harness(language, egui::vec2(1360.0, 900.0));
        h.run();
        let before = h.state().editor.session.snapshot();
        preview(&mut h, language);
        let substitution = translate(
            language,
            paintcraft_ui::i18n::import_loss_description("svg-font-substituted").unwrap(),
        );
        assert!(h.query_by_label(&substitution).is_some());
        consent(&mut h, language);
        upload(h.state_mut(), Ok(Some(vec![font()])));
        h.run();
        assert!(!h.state().pending_svg.as_ref().unwrap().state.accepted);
        assert!(
            h.get_by_role_and_label(egui::Role::Button, &translate(language, "Import image"))
                .accesskit_node()
                .is_disabled()
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        preview(&mut h, language);
        assert!(h.query_by_label(&substitution).is_none());
        consent(&mut h, language);
        h.get_by_role_and_label(egui::Role::Button, &translate(language, "Import image"))
            .click();
        h.run();
        assert!(h.state().pending_svg.is_none());
        assert_eq!(h.state().editor.session.document().canvas, (128, 48));
        let bytes = fixture::font();
        let mut data = paintcraft_ui::fonts::bundled_data().to_vec();
        data.push(&bytes);
        let fonts = svg::Fonts::from_data(&data, "Selawik", &|| Ok(())).unwrap();
        let raster = svg::prepare(
            fixture::TEXT,
            svg::Options {
                width: 128,
                height: 48,
                dpi: 96.0,
                fit: svg::Fit::Contain,
            },
            &fonts,
            svg::PIXELS_MAX,
            &|| Ok(()),
        )
        .unwrap()
        .render(&|| Ok(()))
        .unwrap();
        assert_eq!(
            Sha256::digest(
                paintcraft_render::render_flat(h.state().editor.session.document()).unwrap()
            ),
            Sha256::digest(&raster.rgba)
        );
        assert!(h.state().editor.session.is_dirty());
    }
}

#[test]
fn cancelled_bad_multifile_and_oversized_name_uploads_preserve_the_approved_font_set() {
    let mut h = harness(Language::English, egui::vec2(1360.0, 900.0));
    h.run();
    upload(h.state_mut(), Ok(Some(vec![font()])));
    h.run();
    preview(&mut h, Language::English);
    consent(&mut h, Language::English);
    let before = h.state().editor.session.snapshot();
    let key = h.state().pending_svg.as_ref().unwrap().font_key();
    for files in [
        None,
        Some(vec![]),
        Some(vec![
            font(),
            FontFile {
                name: "bad.ttf".into(),
                bytes: b"invalid font".to_vec(),
            },
        ]),
        Some(vec![FontFile {
            name: "x".repeat(4097),
            bytes: fixture::font(),
        }]),
    ] {
        upload(h.state_mut(), Ok(files));
        h.run();
        let pending = h.state().pending_svg.as_ref().unwrap();
        assert_eq!(pending.font_key(), key);
        assert_eq!(pending.additional.len(), 1);
        assert!(pending.state.accepted);
        assert!(
            !h.get_by_role_and_label(egui::Role::Button, "Import image")
                .accesskit_node()
                .is_disabled()
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    }
    h.get_by_role_and_label(egui::Role::Button, "Remove")
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(egui::Role::Button, "Remove")
        .click();
    h.run();
    assert!(!h.state().pending_svg.as_ref().unwrap().state.accepted);
    assert!(
        h.state()
            .pending_svg
            .as_ref()
            .unwrap()
            .additional
            .is_empty()
    );
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
}

#[test]
fn an_old_font_upload_inbox_cannot_install_into_a_new_source_with_the_same_filename() {
    let mut h = harness(Language::English, egui::vec2(390.0, 640.0));
    h.run();
    let old = h.state().pending_svg.as_ref().unwrap().font_inbox.clone();
    let old_key = h.state().pending_svg.as_ref().unwrap().font_key();
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    let ctx = h.ctx.clone();
    let app = h.state_mut();
    app.import_owner = Some(Owner::capture(app));
    app.accept_svg(
        Source::new(fixture::TEXT.to_vec(), "text.svg".into(), Frontend::Studio).unwrap(),
        &ctx,
    );
    *old.borrow_mut() = Inbox {
        key: Some(old_key),
        result: Some(Ok(Some(vec![font()]))),
    };
    h.run();
    assert!(
        h.state()
            .pending_svg
            .as_ref()
            .unwrap()
            .additional
            .is_empty()
    );
    for label in ["Cancel", "Import image"] {
        assert!(
            h.ctx
                .content_rect()
                .contains_rect(h.get_by_role_and_label(egui::Role::Button, label).rect())
        );
    }
}
