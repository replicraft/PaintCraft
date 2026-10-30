use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_ui::i18n::{Language, set_language, translate};
const CHECK: &str = "Omit original source metadata from this exported copy";
fn app(path: &Path) -> App {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    let mut document = paintcraft_document::new_document(
        128,
        128,
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
    app.editor = Editor::with_document(document, None);
    app.path_text = path.with_extension("pcnd").to_string_lossy().into_owned();
    app.begin_icns_export();
    let state = app.icns_export.as_mut().unwrap();
    state.draft.config.kinds = vec![widget::Kind::Size128];
    app
}
fn harness(app: App, lang: Language) -> Harness<'static, App> {
    Harness::builder()
        .with_size(egui::vec2(1100., 1100.))
        .build_ui_state(
            move |ui, app: &mut App| {
                set_language(ui.ctx(), lang);
                app.confirm_dialog(ui);
            },
            app,
        )
}
#[test]
fn metadata_copy_requires_localized_choice_and_exports_exact_pixels_without_mutation() {
    for lang in Language::ALL {
        let path =
            std::env::temp_dir().join(format!("paintcraft-icns-meta-{}.icns", Uuid::new_v4()));
        let mut h = harness(app(&path), lang);
        let before = h.state().editor.document_snapshot();
        let key = h.state().editor.render_key();
        let dirty = h.state().editor.is_dirty();
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
        let choose = translate(lang, "Choose destination…");
        assert!(h.get_by_label(&choose).accesskit_node().is_disabled());
        h.get_by_label(&translate(lang, CHECK)).click();
        h.run();
        let state = h.state().icns_export.as_ref().unwrap();
        assert_eq!(
            state.cache.value().unwrap().losses(),
            &[export::metadata_policy::OMITTED_REASON.to_owned()]
        );
        assert!(!state.draft.accepted);
        assert!(!h.get_by_label(&choose).accesskit_node().is_disabled());
        h.get_by_label(&choose).click();
        h.run();
        h.get_by_label(&translate(lang, "Export now")).click();
        h.run();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            icns::decode_entry(&bytes, 0, 16384, &|| Ok(()))
                .unwrap()
                .rgba,
            vec![255; 128 * 128 * 4]
        );
        assert!(Arc::ptr_eq(&before, &h.state().editor.document_snapshot()));
        assert_eq!(h.state().editor.render_key(), key);
        assert_eq!(h.state().editor.is_dirty(), dirty);
        assert!(h.state().bound.is_none());
        std::fs::remove_file(path).unwrap();
    }
}
#[test]
fn revoke_and_changed_configuration_cannot_reuse_an_old_picker_policy() {
    let ctx = egui::Context::default();
    let path = std::env::temp_dir().join(format!("paintcraft-icns-policy-{}.icns", Uuid::new_v4()));
    let mut app = app(&path);
    let state = app.icns_export.as_mut().unwrap();
    state.copy.omit = true;
    state.prepare(&ctx);
    let prepared = state.cache.value().unwrap();
    state.bytes = Some(
        prepared
            .encode(&other_losses(prepared.losses()), &|| Ok(()))
            .unwrap(),
    );
    state.copy.omit = false;
    app.complete_icns_picker(&ctx, Some(path.clone()));
    assert!(!path.exists());
    assert!(app.icns_export.is_none());
    app.begin_icns_export();
    let state = app.icns_export.as_mut().unwrap();
    state.draft.config.kinds = vec![widget::Kind::Size128];
    state.copy.omit = true;
    state.prepare(&ctx);
    let prepared = state.cache.value().unwrap();
    state.bytes = Some(
        prepared
            .encode(&other_losses(prepared.losses()), &|| Ok(()))
            .unwrap(),
    );
    state.draft.config.kinds = vec![widget::Kind::Retina16];
    app.complete_icns_picker(&ctx, Some(path.clone()));
    assert!(!path.exists());
    assert!(app.icns_export.is_none());
}
#[test]
fn revoking_choice_retires_preview_and_cancel_preserves_source() {
    let path = Path::new("unused.icns");
    let mut h = harness(app(path), Language::English);
    let source = h.state().editor.document_snapshot();
    h.run();
    h.get_by_label(CHECK).click();
    h.run();
    assert!(
        h.state()
            .icns_export
            .as_ref()
            .unwrap()
            .cache
            .value()
            .is_some()
    );
    h.get_by_label(CHECK).click();
    h.run();
    let state = h.state().icns_export.as_ref().unwrap();
    assert!(state.cache.value().is_none() && state.bytes.is_none());
    assert!(
        state
            .copy
            .validate(&state.draft.config, state.target())
            .is_err()
    );
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().icns_export.is_none());
    assert!(Arc::ptr_eq(&source, &h.state().editor.document_snapshot()));
}

#[test]
fn private_text_is_frozen_once_and_later_edits_revoke_the_asset_copy() {
    let mut app = app(Path::new("unused.icns"));
    app.icns_export = None;
    app.confirm = None;
    app.editor.select_tool(Tool::Text);
    app.editor.pointer_down((2, 2));
    app.editor.pointer_up();
    app.editor.type_text("Ink");
    let before = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    app.capture_icns_export();
    let state = app.icns_export.as_ref().unwrap();
    assert!(state.current(&app));
    assert_ne!(state.copy.frozen.document.state_id, before.state_id);
    assert!(state.copy.frozen.document.source_metadata.is_some());
    let frozen = state.copy.frozen.document.clone();
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
    assert_eq!(ticket, app.editor.history_view().ticket);
    app.editor.type_text(" changed");
    let state = app.icns_export.as_ref().unwrap();
    assert!(!state.current(&app));
    assert!(Arc::ptr_eq(&frozen, &state.copy.frozen.document));
    assert!(Arc::ptr_eq(&before, &app.editor.document_snapshot()));
}

#[test]
fn bake_checkbox_is_separate_from_metadata_and_revokes_old_prepared_copy() {
    let path = std::env::temp_dir().join(format!("paintcraft-asset-bake-{}.icns", Uuid::new_v4()));
    let mut app = app(&path);
    let mut doc = (*app.editor.document_snapshot()).clone();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            Uuid::from_u128(1),
            true,
            paintcraft_core::types::filters::Descriptor::InvertRgbV1,
        )])
        .unwrap();
    app.editor = Editor::with_document(doc, None);
    app.begin_icns_export();
    app.icns_export.as_mut().unwrap().draft.config.kinds = vec![widget::Kind::Size128];
    let original = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let mut h = harness(app, Language::English);
    h.run();
    h.get_by_label(CHECK).click();
    h.run();
    assert!(
        h.state()
            .icns_export
            .as_ref()
            .unwrap()
            .cache
            .value()
            .is_none(),
        "metadata consent cannot authorize baking"
    );
    h.get_by_label("Bake editable filters into this exported copy")
        .click();
    h.run();
    let prepared = h
        .state()
        .icns_export
        .as_ref()
        .unwrap()
        .cache
        .value()
        .unwrap();
    assert_eq!(
        prepared.entries()[0].rgba(),
        [0, 0, 0, 255].repeat(128 * 128)
    );
    assert!(
        prepared
            .losses()
            .iter()
            .any(|s| s == export::filter_bake::BAKED_REASON)
    );
    h.get_by_label("Bake editable filters into this exported copy")
        .click();
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
        h.get_by_label("Choose destination…")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Cancel").click();
    h.run();
    assert!(!path.exists());
    assert!(Arc::ptr_eq(
        &original,
        &h.state().editor.document_snapshot()
    ));
    assert_eq!(ticket, h.state().editor.history_view().ticket);
}
