//! Real Native Context uses document truth, not a fabricated layer count.
use super::*;
use paintcraft_core::types::filters::{BoxBlurV1, Descriptor};
use paintcraft_document::{FilterInstance, FilterStack};
#[test]
fn native_context_offers_single_stored_intent_without_changing_layer_count() {
    for enabled in [false, true] {
        let mut doc = paintcraft_document::new_document(
            2,
            1,
            Some(Rgba([255; 4])),
            0,
            paintcraft_document::POLICY,
        )
        .unwrap();
        doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
            FilterStack::new(vec![FilterInstance::new(
                uuid::Uuid::from_u128(33),
                enabled,
                Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
            )])
            .unwrap();
        let before = doc.clone();
        let mut app = App::new(None);
        app.settings.experience = Experience::Studio;
        app.editor = Editor::with_document(doc, None);
        let context = app.action_context();
        assert_eq!(context.layer_count, 1);
        assert!(context.can_flatten);
        assert!(app.editor.document().same_semantics(&before));
    }
}
