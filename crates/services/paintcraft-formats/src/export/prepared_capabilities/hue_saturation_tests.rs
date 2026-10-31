//! Source-only strict decoder/execution/pricing contract checks.
use super::*;
use paintcraft_core::types::filters::HueSaturationRgbV1;
#[test]
fn hsl_registration_requires_six_signed_bytes_and_priced_direct_contract() {
    let entry = resolve("hue-saturation-rgb-v1", "params-v1").unwrap();
    assert_eq!(entry.execution(), Execution::DirectHueSaturation);
    assert_eq!(
        entry.resource_contract(),
        ("direct-hue-saturation-rgba8-work128-v1", 1)
    );
    assert_eq!(registrations()[10].profile(), "vibrance-rgb-v1");
    assert_eq!(registrations()[11].profile(), "hue-saturation-rgb-v1");
    for values in [
        [-180, -100, -100],
        [0, 0, 0],
        [180, 100, 100],
        [137, 73, -19],
    ] {
        let descriptor = Descriptor::HueSaturationRgbV1(
            HueSaturationRgbV1::new(values[0], values[1], values[2]).unwrap(),
        );
        let (bytes, len) = descriptor.semantic_parameter_bytes();
        assert_eq!(len, 6);
        assert_eq!(entry.decode(&bytes[..len]).unwrap(), descriptor);
        assert!(descriptor_requires_generic(descriptor).unwrap());
    }
    for values in [
        [-181, 0, 0],
        [181, 0, 0],
        [0, -101, 0],
        [0, 101, 0],
        [0, 0, -101],
        [0, 0, 101],
    ] {
        let mut wire = Vec::new();
        for value in values {
            wire.extend_from_slice(&(value as i16).to_be_bytes());
        }
        assert_eq!(
            entry.decode(&wire).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    for len in [0, 1, 5, 7] {
        assert_eq!(
            entry.decode(&vec![0; len]).unwrap_err().code,
            ErrorCode::CodecFailed
        );
    }
    assert_eq!(
        resolve("hue-saturation-rgb-v1", "params-v2")
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
}
#[test]
fn adding_hsl_does_not_change_any_old_required_subset_digest() {
    let mut old = Requirements::default();
    for id in [
        "invert-rgb-v1",
        "brightness-contrast-v1",
        "black-white-rgb-v1",
        "threshold-luma-v1",
        "posterize-rgb-v1",
        "solarize-rgb-v1",
        "box-blur-v1",
        "levels-rgb-v1",
        "exposure-srgb-v1",
        "curves-rgb-v1",
        "vibrance-rgb-v1",
    ] {
        old.insert(id, "params-v1").unwrap();
    }
    let before = old.digests(&|| Ok(())).unwrap();
    assert_eq!(
        before.0,
        [
            208, 94, 136, 59, 71, 173, 193, 67, 48, 81, 92, 33, 137, 237, 200, 133, 13, 99, 81,
            158, 10, 233, 121, 109, 124, 242, 93, 78, 106, 51, 147, 77
        ]
    );
    assert_eq!(
        before.1,
        [
            24, 221, 99, 143, 181, 79, 35, 245, 218, 120, 245, 78, 212, 207, 9, 40, 197, 60, 247,
            144, 183, 105, 203, 80, 156, 138, 52, 148, 95, 171, 192, 168
        ]
    );
    let mut hsl = Requirements::default();
    hsl.insert("hue-saturation-rgb-v1", "params-v1").unwrap();
    assert_ne!(hsl.digests(&|| Ok(())).unwrap(), before);
    assert_eq!(old.digests(&|| Ok(())).unwrap(), before);
    let d = Descriptor::HueSaturationRgbV1(HueSaturationRgbV1::default());
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(paintcraft_core::types::Rgba([0, 0, 0, 255])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    doc.layers.get_mut(&doc.roots[0]).unwrap().filters =
        paintcraft_document::FilterStack::new(vec![paintcraft_document::FilterInstance::new(
            uuid::Uuid::from_u128(128),
            false,
            d,
        )])
        .unwrap();
    assert!(requires_generic(&doc, &|| Ok(())).unwrap());
    assert_eq!(
        Requirements::capture(&doc, &|| Ok(()))
            .unwrap()
            .pairs()
            .collect::<Vec<_>>(),
        vec![("hue-saturation-rgb-v1", "params-v1")]
    );
}
