use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, format_message, set_language, translate};
const CHECK: &str = "Omit original source metadata from this exported copy";
#[test]
fn accepted_metadata_choice_encodes_icns_and_reaches_actual_file_publication_boundary() {
    let app = app(false);
    let original = app.editor.session.snapshot();
    let ticket = app.editor.session.history_ticket();
    let mut h = harness(app, Language::English);
    h.run();
    h.get_by_label(CHECK).click();
    h.run();
    let state = h.state().icns_export.as_ref().unwrap();
    let prepared = state.cache.value().unwrap();
    assert!(prepared.losses().iter().any(|loss| loss == OMITTED_REASON));
    let bytes = prepared.encode(&state.other_losses, &|| Ok(())).unwrap();
    assert_eq!(&bytes[..4], b"icns");
    h.get_by_label(&download(Language::English)).click();
    h.run();
    assert!(h.state().icns_export.is_none());
    let result = h
        .state_mut()
        .files
        .poll()
        .expect("download seam was called");
    assert_eq!(
        result.unwrap_err().code,
        paintcraft_core::types::ErrorCode::CapabilityUnsupported
    );
    assert!(!h.state().message.contains("unknown icon export loss"));
    assert!(std::sync::Arc::ptr_eq(
        &original,
        &h.state().editor.session.snapshot()
    ));
    assert_eq!(ticket, h.state().editor.session.history_ticket());
}
fn app(text: bool) -> WebApp {
    let mut app = WebApp::new(&egui::Context::default());
    app.frontend = Frontend::Studio;
    let mut document = paintcraft_document::new_document(
        32,
        32,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    document.source_metadata = paintcraft_formats::pcnd::read(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    )))
    .unwrap()
    .source_metadata;
    app.editor = Editor::from_document(document).unwrap();
    if text {
        app.controls.tool = Tool::Text;
        app.editor
            .tools
            .begin_text(app.editor.target.unwrap(), (4, 4));
        app.editor.tools.type_text("Once");
    }
    app.begin_icns_export();
    let state = app.icns_export.as_mut().unwrap();
    state.draft.config.kinds = vec![widget::Kind::Retina16];
    state.config = state.draft.config.clone();
    app
}
fn harness(app: WebApp, lang: Language) -> Harness<'static, WebApp> {
    Harness::builder()
        .with_size(egui::vec2(1200., 1100.))
        .build_ui_state(
            move |ui, app: &mut WebApp| {
                set_language(ui.ctx(), lang);
                app.icns_export_dialog(ui.ctx());
            },
            app,
        )
}
fn download(lang: Language) -> String {
    format_message(lang, "Download { $format }", &[("format", "ICNS".into())])
}
#[test]
fn all_locales_require_metadata_choice_before_icns_preparation_and_revoke_the_cache() {
    for lang in Language::ALL {
        let mut h = harness(app(false), lang);
        let original = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.history_ticket();
        h.run();
        assert!(
            h.state()
                .icns_export
                .as_ref()
                .unwrap()
                .cache
                .value()
                .is_none()
        );
        assert!(
            h.get_by_label(&download(lang))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&translate(lang, CHECK)).click();
        h.run();
        let state = h.state().icns_export.as_ref().unwrap();
        assert!(state.cache.value().is_some() && state.authorized());
        assert!(state.other_losses.is_empty());
        assert!(
            h.query_by_label(&translate(lang, "I accept these export changes."))
                .is_none()
        );
        assert!(
            !h.get_by_label(&download(lang))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&translate(lang, CHECK)).click();
        h.run();
        let state = h.state().icns_export.as_ref().unwrap();
        assert!(state.cache.value().is_none() && state.policy.is_none());
        assert!(!state.authorized());
        h.get_by_label(&translate(lang, "Cancel")).click();
        h.run();
        assert!(h.state().icns_export.is_none());
        assert!(std::sync::Arc::ptr_eq(
            &original,
            &h.state().editor.session.snapshot()
        ));
        assert_eq!(ticket, h.state().editor.session.history_ticket());
    }
}
#[test]
fn icns_uses_one_private_text_snapshot_config_reconsent_and_drops_stale_private_pixels() {
    let mut h = harness(app(true), Language::English);
    let original = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.history_ticket();
    let frozen = h
        .state()
        .icns_export
        .as_ref()
        .unwrap()
        .frozen
        .document
        .clone();
    let weak = std::sync::Arc::downgrade(&frozen);
    assert_ne!(frozen.state_id, original.state_id);
    let expected = paintcraft_render::render_flat(&frozen).unwrap();
    h.run();
    h.get_by_label(CHECK).click();
    h.run();
    let state = h.state().icns_export.as_ref().unwrap();
    assert!(std::sync::Arc::ptr_eq(&frozen, &state.frozen.document));
    assert_eq!(state.cache.value().unwrap().entries()[0].rgba(), expected);
    drop(frozen);
    h.state_mut()
        .icns_export
        .as_mut()
        .unwrap()
        .draft
        .config
        .kinds = vec![widget::Kind::Retina32];
    h.run();
    let state = h.state().icns_export.as_ref().unwrap();
    assert!(!state.omit_metadata && state.cache.value().is_none() && state.policy.is_none());
    h.get_by_label(CHECK).click();
    h.run();
    assert!(
        h.state()
            .icns_export
            .as_ref()
            .unwrap()
            .other_losses
            .iter()
            .any(|loss| loss == "icon-resized")
    );
    assert!(
        h.get_by_label(&download(Language::English))
            .accesskit_node()
            .is_disabled(),
        "other losses still need their own consent"
    );
    h.state_mut().editor.tools.type_text(" changed");
    h.run();
    assert!(h.state().icns_export.is_none());
    assert!(weak.upgrade().is_none());
    assert!(std::sync::Arc::ptr_eq(
        &original,
        &h.state().editor.session.snapshot()
    ));
    assert_eq!(ticket, h.state().editor.session.history_ticket());
    assert_eq!(h.state().editor.tools.text_content(), Some("Once changed"));
    assert!(
        h.state()
            .editor
            .session
            .document()
            .source_metadata
            .is_some()
    );
}
