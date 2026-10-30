use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, format_message, set_language, translate};
const CHECK: &str = "Omit original source metadata from this exported copy";
#[test]
fn accepted_metadata_choice_encodes_both_kinds_and_reaches_actual_file_publication_boundary() {
    for kind in [widget::Kind::Icon, widget::Kind::Cursor] {
        let mut app = app(false);
        let state = app.icon_export.as_mut().unwrap();
        state.draft.config.kind = kind;
        state.config = state.draft.config.clone();
        let original = app.editor.session.snapshot();
        let ticket = app.editor.session.history_ticket();
        let mut h = harness(app, Language::English);
        h.run();
        h.get_by_label(CHECK).click();
        h.run();
        let state = h.state().icon_export.as_ref().unwrap();
        let prepared = state.cache.value().unwrap();
        assert!(prepared.losses().iter().any(|loss| loss == OMITTED_REASON));
        let bytes = prepared.encode(&state.other_losses, &|| Ok(())).unwrap();
        assert_eq!(
            &bytes[..4],
            if kind == widget::Kind::Icon {
                &[0, 0, 1, 0]
            } else {
                &[0, 0, 2, 0]
            }
        );
        h.get_by_label(&download(Language::English, kind.name()))
            .click();
        h.run();
        assert!(h.state().icon_export.is_none());
        // Host builds deliberately refuse actual browser downloads. Reaching this
        // error proves the real dialog encoded and called the publication seam.
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
}
fn app(text: bool) -> WebApp {
    let mut app = WebApp::new(&egui::Context::default());
    app.frontend = Frontend::Studio;
    let mut document = paintcraft_document::new_document(
        if text { 160 } else { 2 },
        if text { 64 } else { 2 },
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
    app.begin_icon_export();
    let state = app.icon_export.as_mut().unwrap();
    state.draft.config.entries = vec![widget::Entry {
        width: if text { 160 } else { 2 },
        height: if text { 64 } else { 2 },
        hotspot: (0, 0),
        encoding: widget::Encoding::Png,
    }];
    state.config = state.draft.config.clone();
    app
}
fn harness(app: WebApp, lang: Language) -> Harness<'static, WebApp> {
    Harness::builder()
        .with_size(egui::vec2(1200., 1100.))
        .build_ui_state(
            move |ui, app: &mut WebApp| {
                set_language(ui.ctx(), lang);
                app.icon_export_dialog(ui.ctx());
            },
            app,
        )
}
fn download(lang: Language, kind: &str) -> String {
    format_message(lang, "Download { $format }", &[("format", kind.into())])
}
#[test]
fn localized_metadata_choice_is_required_before_preparation_and_revoke_retires_authority() {
    for lang in Language::ALL {
        let mut h = harness(app(false), lang);
        let source = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.history_ticket();
        h.run();
        assert!(
            h.state()
                .icon_export
                .as_ref()
                .unwrap()
                .cache
                .value()
                .is_none()
        );
        assert!(
            h.get_by_label(&download(lang, "ICO"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&translate(lang, CHECK)).click();
        h.run();
        let state = h.state().icon_export.as_ref().unwrap();
        assert!(state.cache.value().is_some());
        assert!(state.authorized());
        assert!(state.other_losses.is_empty());
        assert!(
            h.query_by_label(&translate(lang, "I accept these export changes."))
                .is_none(),
            "metadata intent needs no duplicate loss consent"
        );
        assert!(
            !h.get_by_label(&download(lang, "ICO"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&translate(lang, CHECK)).click();
        h.run();
        let state = h.state().icon_export.as_ref().unwrap();
        assert!(state.cache.value().is_none() && state.policy.is_none());
        assert!(!state.authorized());
        assert!(
            h.get_by_label(&download(lang, "ICO"))
                .accesskit_node()
                .is_disabled()
        );
        h.get_by_label(&translate(lang, "Cancel")).click();
        h.run();
        assert!(h.state().icon_export.is_none());
        assert!(std::sync::Arc::ptr_eq(
            &source,
            &h.state().editor.session.snapshot()
        ));
        assert_eq!(ticket, h.state().editor.session.history_ticket());
    }
}
#[test]
fn text_is_frozen_once_and_kind_config_changes_require_fresh_consent_without_publication() {
    let mut h = harness(app(true), Language::English);
    let original = h.state().editor.session.snapshot();
    let ticket = h.state().editor.session.history_ticket();
    let frozen = h
        .state()
        .icon_export
        .as_ref()
        .unwrap()
        .frozen
        .document
        .clone();
    assert_ne!(frozen.state_id, original.state_id);
    let expected = paintcraft_render::render_flat(&frozen).unwrap();
    h.run();
    h.get_by_label(CHECK).click();
    h.run();
    let state = h.state().icon_export.as_ref().unwrap();
    assert!(std::sync::Arc::ptr_eq(&frozen, &state.frozen.document));
    assert_eq!(state.cache.value().unwrap().entries()[0].rgba(), expected);
    let ico_policy = state.policy.unwrap();
    assert!(
        ico_policy
            .source_metadata
            .validate(&frozen, state.target())
            .unwrap()
    );
    let checks = std::cell::Cell::new(0);
    let check = || {
        checks.set(checks.get() + 1);
        Ok(())
    };
    assert!(
        !ico_policy
            .filter_bake
            .validate(&frozen, state.target(), &check)
            .unwrap()
    );
    assert!(checks.get() > 0);
    h.state_mut()
        .icon_export
        .as_mut()
        .unwrap()
        .draft
        .config
        .kind = widget::Kind::Cursor;
    h.run();
    let state = h.state().icon_export.as_ref().unwrap();
    assert!(!state.omit_metadata && state.policy.is_none() && state.cache.value().is_none());
    assert!(
        ico_policy
            .source_metadata
            .validate(&frozen, state.target())
            .is_err()
    );
    // No-filter Refuse grants no bake authority; omission's original ICO
    // witness still refuses retargeting to CUR above.
    assert!(
        !ico_policy
            .filter_bake
            .validate(&frozen, state.target(), &check)
            .unwrap()
    );
    h.get_by_label(CHECK).click();
    h.run();
    assert!(h.state().icon_export.as_ref().unwrap().authorized());
    h.state_mut().editor.tools.type_text(" changed");
    h.run();
    assert!(h.state().icon_export.is_none());
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
