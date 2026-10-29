use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use paintcraft_formats::export::metadata_policy::{Intent, OMITTED_REASON};
use paintcraft_ui::i18n::{Language, set_language, translate};
const CHECK: &str = "Omit original source metadata from this exported copy";
fn setup(ctx: &egui::Context) -> WebApp {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ));
    let attachment = paintcraft_formats::pcnd::read(bytes)
        .unwrap()
        .source_metadata;
    let mut doc = paintcraft_document::new_document(
        160,
        64,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.source_metadata = attachment;
    let mut app = WebApp::new(ctx);
    app.editor = crate::editor::Editor::from_document(doc).unwrap();
    app.export_confirm = Some(ImageExport::Png);
    app
}
#[test]
fn web_metadata_checkbox_is_localized_required_and_cancel_drops_frozen_authority() {
    for language in Language::ALL {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1200., 1000.))
            .build_ui_state(
                move |ui, app: &mut WebApp| {
                    set_language(ui.ctx(), language);
                    app.export_dialog(ui.ctx());
                },
                setup(&egui::Context::default()),
            );
        let original = h.state().editor.session.snapshot();
        let ticket = h.state().editor.session.history_ticket();
        h.run();
        let tr = |source| translate(language, source);
        let download = paintcraft_ui::i18n::format_message(
            language,
            "Download { $format }",
            &[("format", "PNG".into())],
        );
        assert!(h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_label(&tr(CHECK)).click();
        h.run();
        assert!(!h.get_by_label(&download).accesskit_node().is_disabled());
        h.get_by_label(&tr("Cancel")).click();
        h.run();
        assert!(h.state().export_frozen.is_none());
        assert!(!h.state().export_omit_source_metadata);
        assert!(std::sync::Arc::ptr_eq(
            &original,
            &h.state().editor.session.snapshot()
        ));
        assert_eq!(ticket, h.state().editor.session.history_ticket());
    }
}
#[test]
fn web_private_text_copy_uses_same_snapshot_for_policy_and_pixels_and_detects_changed_draft() {
    let mut app = setup(&egui::Context::default());
    app.controls.tool = Tool::Text;
    app.editor
        .tools
        .begin_text(app.editor.target.unwrap(), (4, 4));
    app.editor.tools.type_text("Ink");
    let original = app.editor.session.snapshot();
    let ticket = app.editor.session.history_ticket();
    let frozen = Frozen::capture(&mut app).unwrap();
    assert_ne!(frozen.document.state_id, original.state_id);
    assert!(frozen.current(&app));
    let expected = paintcraft_render::render_flat(&frozen.document).unwrap();
    let target = paintcraft_formats::export::metadata_policy::Target::new(
        paintcraft_formats::registry::Family::Png,
        paintcraft_formats::registry::Operation::ExportFlat,
    );
    let policy = paintcraft_formats::export::metadata_policy::Policy::capture(
        &frozen.document,
        target,
        Intent::Omit,
    )
    .unwrap();
    let mut options = paintcraft_formats::export::FlatOptions::new(
        paintcraft_formats::image::ImageFormat::Png,
        90,
    );
    options.source_metadata = policy;
    let flat =
        paintcraft_formats::export::export_flat_options(&frozen.document, options, &[]).unwrap();
    assert_eq!(flat.losses, [OMITTED_REASON]);
    assert_eq!(
        paintcraft_formats::image::decode(&flat.bytes).unwrap().rgba,
        expected
    );
    assert!(
        policy.validate(&original, target).is_err(),
        "private export state differs from live state"
    );
    assert!(std::sync::Arc::ptr_eq(
        &original,
        &app.editor.session.snapshot()
    ));
    assert_eq!(ticket, app.editor.session.history_ticket());
    assert_eq!(app.editor.tools.text_content(), Some("Ink"));
    app.editor.tools.type_text("changed");
    assert!(!frozen.current(&app));
}
