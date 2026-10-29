use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, format_message, translate};
use std::sync::Arc;
fn label(lang: Language, kind: widget::Kind) -> String {
    format_message(
        lang,
        "{ $tag }: { $logical } × { $logical } at { $scale }×, { $pixels } × { $pixels } px",
        &[
            ("tag", kind.tag().into()),
            ("logical", kind.logical_size().to_string()),
            ("scale", kind.scale().to_string()),
            ("pixels", kind.pixel_size().to_string()),
        ],
    )
}
fn setup(lang: Language) -> Harness<'static, WebApp> {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    app.frontend = Frontend::Studio;
    app.editor = Editor::from_document(
        paintcraft_document::import_image(2, 1, &[255, 0, 0, 255, 0, 0, 255, 255], "icns-web-ui")
            .unwrap(),
    )
    .unwrap();
    app.name = "painting.pcnd".into();
    app.begin_icns_export();
    Harness::builder()
        .with_max_steps(30)
        .with_size(egui::vec2(1000.0, 1100.0))
        .build_ui_state(
            move |ui, app: &mut WebApp| {
                if ui.ctx().missing_glyph_policy() != egui::epaint::text::MissingGlyphPolicy::Tofu {
                    paintcraft_ui::fonts::install(ui.ctx());
                    ui.ctx()
                        .set_missing_glyph_policy(egui::epaint::text::MissingGlyphPolicy::Tofu);
                }
                paintcraft_ui::i18n::set_language(ui.ctx(), lang);
                app.dialogs(ui.ctx());
            },
            app,
        )
}
#[test]
fn browser_icns_export_uses_explicit_typed_preview_buffers_and_preserves_painting() {
    for lang in [Language::English, Language::Japanese] {
        let mut h = setup(lang);
        h.run();
        let before = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.ticket();
        let dirty = h.state().editor.session.is_dirty();
        let download = format_message(lang, "Download { $format }", &[("format", "ICNS".into())]);
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        for kind in [widget::Kind::Size128, widget::Kind::Retina16] {
            h.get_by_role_and_label(egui::Role::CheckBox, &label(lang, kind))
                .click();
            h.run();
        }
        let consent = translate(lang, "I accept these export changes.");
        h.get_by_role_and_label(egui::Role::CheckBox, &consent)
            .scroll_to_me();
        h.run();
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_role_and_label(egui::Role::CheckBox, &consent)
            .click();
        h.run();
        assert!(!h.get_by_label(&download).accesskit_node().is_disabled());
        let prepared = h
            .state()
            .icns_export
            .as_ref()
            .unwrap()
            .cache
            .value()
            .unwrap();
        let bytes = prepared.encode(prepared.losses(), &|| Ok(())).unwrap();
        let directory = icns::inspect(&bytes, &|| Ok(())).unwrap();
        assert_eq!(
            directory.entries.iter().map(|e| e.tag).collect::<Vec<_>>(),
            [*b"ic07", *b"ic11"]
        );
        for (index, entry) in prepared.entries().iter().enumerate() {
            let decoded = icns::decode_entry(&bytes, index as u16, 4_194_304, &|| Ok(())).unwrap();
            assert_eq!(decoded.rgba, entry.rgba());
            assert_eq!(decoded.entry.width, entry.kind.pixel_size());
        }
        h.get_by_role_and_label(egui::Role::Button, &download)
            .click();
        h.run();
        assert!(h.state().icns_export.is_none());
        assert!(h.state().files.busy);
        assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
        assert_eq!(ticket, h.state().editor.session.ticket());
        assert_eq!(dirty, h.state().editor.session.is_dirty());
        assert_eq!(h.state().name, "painting.pcnd");
    }
}
#[test]
fn changed_export_options_stale_owner_cancel_and_classic_cannot_download_icns() {
    let mut h = setup(Language::English);
    h.run();
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &label(Language::English, widget::Kind::Retina16),
    )
    .click();
    h.run();
    let consent = "I accept these export changes.";
    h.get_by_role_and_label(egui::Role::CheckBox, consent)
        .scroll_to_me();
    h.run();
    h.get_by_role_and_label(egui::Role::CheckBox, consent)
        .click();
    h.run();
    assert!(
        !h.get_by_label("Download ICNS")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &label(Language::English, widget::Kind::Retina32),
    )
    .scroll_to_me();
    h.run();
    h.get_by_role_and_label(
        egui::Role::CheckBox,
        &label(Language::English, widget::Kind::Retina32),
    )
    .click();
    h.run();
    assert!(
        h.get_by_label("Download ICNS")
            .accesskit_node()
            .is_disabled()
    );
    let before = h.state().editor.session.snapshot();
    h.get_by_role_and_label(egui::Role::Button, "Cancel")
        .click();
    h.run();
    assert!(h.state().icns_export.is_none());
    assert!(!h.state().files.busy);
    assert!(Arc::ptr_eq(&before, &h.state().editor.session.snapshot()));
    h.state_mut().begin_icns_export();
    h.state_mut().editor = Editor::new(3, 3).unwrap();
    let current = h.state().editor.session.snapshot();
    h.run();
    assert!(h.state().icns_export.is_none());
    assert!(Arc::ptr_eq(&current, &h.state().editor.session.snapshot()));
    h.state_mut().frontend = Frontend::Classic;
    h.state_mut().begin_icns_export();
    assert!(h.state().icns_export.is_none());
}
