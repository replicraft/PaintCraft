use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, format_message, translate};
const SOURCE: &[u8] = include_bytes!(
    "../../../../../../crates/services/paintcraft-formats/tests/icns-fixtures/modern.icns"
);
fn setup(lang: Language) -> Harness<'static, WebApp> {
    let ctx = eframe::egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = paintcraft_ui::actions::Frontend::Studio;
    app.import_owner = Some(Owner::capture(&app));
    app.accept_icns(
        Source::new(SOURCE.to_vec(), "source.icns".into(), Frontend::Studio).unwrap(),
        &ctx,
    );
    Harness::builder()
        .with_max_steps(30)
        .with_size(eframe::egui::vec2(1100.0, 1000.0))
        .build_ui_state(
            move |ui, app: &mut WebApp| {
                if ui.ctx().missing_glyph_policy()
                    != eframe::egui::epaint::text::MissingGlyphPolicy::Tofu
                {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx().set_missing_glyph_policy(
                        eframe::egui::epaint::text::MissingGlyphPolicy::Tofu,
                    );
                }
                paintcraft_ui::i18n::set_language(ui.ctx(), lang);
                app.icns_import_dialog(ui.ctx());
            },
            app,
        )
}
fn select(h: &mut Harness<WebApp>, lang: Language, index: u16, tag: &str) {
    let label = format_message(
        lang,
        "Entry { $number }: { $tag }",
        &[("number", (index + 1).to_string()), ("tag", tag.into())],
    );
    h.get_by_role_and_label(eframe::egui::Role::RadioButton, &label)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::RadioButton, &label)
        .click();
    h.run();
    h.get_by_role_and_label(
        eframe::egui::Role::Button,
        &translate(lang, "Update preview"),
    )
    .scroll_to_me();
    h.run();
    h.get_by_role_and_label(
        eframe::egui::Role::Button,
        &translate(lang, "Update preview"),
    )
    .click();
    h.run();
}
fn approve(h: &mut Harness<WebApp>, lang: Language) {
    let text = translate(lang, "I accept these import changes.");
    h.get_by_role_and_label(eframe::egui::Role::CheckBox, &text)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(eframe::egui::Role::CheckBox, &text)
        .click();
    h.run();
}
#[test]
fn translated_browser_icns_entry_preview_and_consent_match_exact_native_density_pixels() {
    for lang in [Language::English, Language::Japanese] {
        let mut h = setup(lang);
        h.run();
        let before = h.state().editor.session.snapshot();
        assert!(
            h.get_by_label(&translate(lang, "Import selected entry"))
                .accesskit_node()
                .is_disabled()
        );
        select(&mut h, lang, 6, "ic11");
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert!(
            h.get_by_label(&translate(lang, "Import selected entry"))
                .accesskit_node()
                .is_disabled()
        );
        approve(&mut h, lang);
        h.get_by_role_and_label(
            eframe::egui::Role::Button,
            &translate(lang, "Import selected entry"),
        )
        .click();
        h.run();
        assert!(h.state().pending_icns.is_none());
        assert_eq!(h.state().editor.session.document().canvas, (32, 32));
        assert!(h.state().editor.session.is_dirty());
        assert_eq!(h.state().editor.session.undo_depth(), 0);
        let expected = icns::decode_entry(SOURCE, 6, 4096, &|| Ok(())).unwrap();
        let doc = h.state().editor.session.document();
        let layer = &doc.layers[&doc.roots[0]];
        for (i, rgba) in expected.rgba.as_chunks::<4>().0.iter().enumerate() {
            assert_eq!(layer.raster.get((i % 32) as i64, (i / 32) as i64).0, *rgba);
        }
    }
}
#[test]
fn browser_changed_entry_cancel_stale_owner_and_classic_callbacks_cannot_import_previous_preview() {
    let mut h = setup(Language::English);
    h.run();
    let before = h.state().editor.session.snapshot();
    select(&mut h, Language::English, 6, "ic11");
    approve(&mut h, Language::English);
    select(&mut h, Language::English, 7, "ic12");
    assert!(
        h.get_by_label("Import selected entry")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_role_and_label(eframe::egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().pending_icns.is_none());
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    let ctx = h.ctx.clone();
    let owner = Owner::capture(h.state());
    h.state_mut().import_owner = Some(owner);
    h.state_mut().accept_icns(
        Source::new(SOURCE.to_vec(), "source.icns".into(), Frontend::Studio).unwrap(),
        &ctx,
    );
    h.state_mut().editor = crate::editor::Editor::new(3, 3).unwrap();
    let current = h.state().editor.session.snapshot();
    h.run();
    assert!(h.state().pending_icns.is_none());
    assert!(Arc::ptr_eq(&current, &h.state().editor.session.snapshot()));
    assert!(Source::new(SOURCE.to_vec(), "source.icns".into(), Frontend::ClassicV11).is_err());
    assert!(Source::new(SOURCE.to_vec(), "source.png".into(), Frontend::Studio).is_err());
}
