use super::*;
use paintcraft_core::types::filters::{
    BlackWhiteV1, BrightnessContrastV1, PosterizeRgbV1, ThresholdLumaV1,
};
fn stack(descriptor: Descriptor) -> FilterStack {
    FilterStack::new(vec![FilterInstance::new(Uuid::nil(), true, descriptor)]).unwrap()
}
#[test]
fn every_point_parameter_and_variant_binds_stack_identity() {
    let mut seen = std::collections::HashSet::new();
    for value in 0..=100 {
        assert!(
            seen.insert(
                stack(Descriptor::BlackWhiteRgbV1(
                    BlackWhiteV1::new(value).unwrap()
                ))
                .digest()
            )
        );
    }
    for value in 0..=255 {
        assert!(
            seen.insert(
                stack(Descriptor::ThresholdLumaV1(
                    ThresholdLumaV1::new(value).unwrap()
                ))
                .digest()
            )
        );
    }
    for value in 2..=32 {
        assert!(
            seen.insert(
                stack(Descriptor::PosterizeRgbV1(
                    PosterizeRgbV1::new(value).unwrap()
                ))
                .digest()
            )
        );
    }
    assert!(seen.insert(stack(Descriptor::SolarizeRgbV1).digest()));
    assert!(seen.insert(stack(Descriptor::InvertRgbV1).digest()));
    assert!(
        seen.insert(
            stack(Descriptor::BrightnessContrastV1(
                BrightnessContrastV1::new(0, 0).unwrap()
            ))
            .digest()
        )
    );
}
#[test]
fn old_invert_and_bc_digest_bytes_remain_independent_literal_vectors() {
    // Python hashlib reference of the already-frozen v1 canonical field stream.
    for (descriptor, expected) in [
        (
            Descriptor::InvertRgbV1,
            "3cfb2108a9b81c1d46bf9f8d260801ee2a36d878215bea720dc88da74cc6da2b",
        ),
        (
            Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-19, 23).unwrap()),
            "7282596b7c38dba714b1e5702b33dc1e0d6a789be5524e362fe1747c687d261f",
        ),
    ] {
        assert_eq!(crate::hex(&stack(descriptor).digest()), expected);
    }
}
