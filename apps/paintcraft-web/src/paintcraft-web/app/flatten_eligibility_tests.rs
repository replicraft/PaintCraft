//! Real Main Context exposes a single filtered document while count stays one.
use super::*;
use paintcraft_core::types::filters::{AddNoiseRgbV1, Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
use uuid::Uuid;
#[test]
fn web_context_offers_single_disabled_noise_intent_without_changing_layer_count() {
    let ctx = egui::Context::default();
    let mut app = WebApp::new(&ctx);
    let mut doc = paintcraft_document::new_document(
        2,
        1,
        Some(paintcraft_core::types::Rgba([255; 4])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        FilterStack::new(vec![FilterInstance::new(
            Uuid::from_u128(34),
            false,
            Descriptor::AddNoiseRgbV1(AddNoiseRgbV1::default()),
        )])
        .unwrap();
    let before = doc.clone();
    app.frontend = Frontend::Studio;
    app.editor = Editor::from_document(doc).unwrap();
    let context = app.context();
    assert_eq!(context.layer_count, 1);
    assert!(context.can_flatten);
    assert!(app.editor.session.document().same_semantics(&before));
    assert_eq!(app.editor.session.undo_depth(), 0);
}
