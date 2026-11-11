use super::*;
use paintcraft_core::types::{
    Rgba,
    styles::{ColorOverlayRgbV1, EffectBlendMode},
};
#[test]
fn constant_empty_matches_canonical_digest_and_no_intent_is_erased() {
    const EMPTY: StyleStack = StyleStack::empty();
    assert_eq!(EMPTY.digest(), hash(&[], &|| Ok(())).unwrap());
    assert_eq!(EMPTY, StyleStack::default());
    assert_eq!(EMPTY.storage_bytes(), 0);
    for enabled in [false, true] {
        let mut doc =
            crate::new_document(1, 1, Some(Rgba([7, 9, 11, 0])), 0, crate::POLICY).unwrap();
        let old = doc.clone();
        let layer = doc.roots[0];
        let style = StyleInstance::new(
            Uuid::from_u128(1),
            enabled,
            StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                [1, 2, 3],
                0,
                EffectBlendMode::Screen,
            )),
        )
        .unwrap();
        doc.layers.get_mut(&layer).unwrap().styles = StyleStack::new(vec![style]).unwrap();
        assert!(doc.has_styles());
        assert!(doc.requires_flatten());
        assert!(!doc.same_semantics(&old));
        assert_eq!(
            doc.require_no_styles().unwrap_err().code,
            ErrorCode::CapabilityUnsupported
        );
        let copy = doc.layers[&layer]
            .styles
            .try_clone_checked(&|| Ok(()))
            .unwrap();
        assert_eq!(copy.digest(), doc.layers[&layer].styles.digest());
        assert_eq!(copy.as_slice(), &[style]);
    }
}
#[test]
fn fallible_copy_cancellation_preserves_original_complete_intent() {
    let style = StyleInstance::new(
        Uuid::from_u128(1),
        false,
        StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
            [19, 31, 47],
            0,
            EffectBlendMode::Multiply,
        )),
    )
    .unwrap();
    let stack = StyleStack::new(vec![style]).unwrap();
    let digest = stack.digest();
    let error = Error::new(ErrorCode::ResourceLimit, "style clone sentinel");
    let refused = stack.try_clone_checked(&|| Err(error.clone())).unwrap_err();
    assert_eq!(refused.code, error.code);
    assert_eq!(refused.message, error.message);
    assert_eq!(stack.digest(), digest);
    assert_eq!(stack.as_slice(), &[style]);
}
