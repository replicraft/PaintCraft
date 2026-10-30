//! Unchecked editable-intention consent never starts the external writer.
use super::*;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
#[test]
fn unchecked_bake_and_metadata_omission_cannot_start_jxr_helper() {
    let mut app = App::new(None);
    app.settings.experience = Experience::Studio;
    app.initialize_editing(app.context.clone());
    let mut doc = paintcraft_document::new_document(
        4,
        3,
        Some(Rgba([0, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            Uuid::from_u128(1),
            true,
            paintcraft_core::types::filters::Descriptor::InvertRgbV1,
        )])
        .unwrap();
    doc.source_metadata=paintcraft_formats::pcnd::read(include_bytes!("../../../../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd")).unwrap().source_metadata;
    app.editor = Editor::with_document(doc, None);
    let original = app.editor.document_snapshot();
    let ticket = app.editor.history_view().ticket;
    let frozen = export_options::Frozen::capture(&app).unwrap();
    app.begin_jxr_export_frozen(frozen, Intent::Refuse);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1100., 1000.))
        .build_ui_state(
            |ui, app: &mut App| {
                app.poll_jxr_export(ui.ctx());
                app.confirm_dialog(ui);
            },
            app,
        );
    h.run();
    assert!(
        h.get_by_label("Choose destination…")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Omit original source metadata from this exported copy")
        .click();
    h.run();
    let state = h.state().jxr_export.as_ref().unwrap();
    assert!(!state.bake_filters && state.pending.is_none() && state.prepared.is_none());
    assert_eq!(h.state().worker_budget.usage().reserved, 0);
    assert!(
        h.get_by_label("Choose destination…")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Cancel").click();
    h.run();
    assert!(h.state().jxr_export.is_none());
    assert!(Arc::ptr_eq(
        &original,
        &h.state().editor.document_snapshot()
    ));
    assert_eq!(ticket, h.state().editor.history_view().ticket);
}
