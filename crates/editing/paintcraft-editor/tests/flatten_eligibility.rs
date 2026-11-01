//! Predicate/transaction coupling, not an expected renderer implementation.
use paintcraft_core::types::{
    Rgba,
    filters::{BoxBlurV1, Descriptor},
};
use paintcraft_document::{FilterInstance, FilterStack, POLICY, new_document};
use paintcraft_editor::session::Session;
use std::cell::Cell;
#[test]
fn editor_uses_document_predicate_for_noop_and_single_stored_intent() {
    for enabled in [None, Some(false), Some(true)] {
        let mut doc = new_document(2, 1, Some(Rgba([255; 4])), 0, POLICY).unwrap();
        if let Some(enabled) = enabled {
            doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
                FilterStack::new(vec![FilterInstance::new(
                    uuid::Uuid::from_u128(77),
                    enabled,
                    Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()),
                )])
                .unwrap();
        }
        let requires = doc.requires_flatten();
        let before = doc.clone();
        let mut session = Session::new(doc);
        let calls = Cell::new(0u32);
        let receipt = session
            .flatten(|captured, _| {
                calls.set(calls.get() + 1);
                assert!(captured.requires_flatten());
                Ok(vec![255; 8])
            })
            .unwrap();
        assert_eq!(receipt.committed, requires);
        assert_eq!(calls.get(), u32::from(requires));
        assert_eq!(session.undo_depth(), usize::from(requires));
        assert!(!session.document().requires_flatten());
        if requires {
            session.undo().unwrap();
            assert!(session.document().same_semantics(&before));
            assert!(session.document().requires_flatten());
            session.redo().unwrap();
            assert!(!session.document().requires_flatten());
        }
    }
}
