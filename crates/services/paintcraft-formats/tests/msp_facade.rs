use paintcraft_core::types::{ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::{
    export,
    image::{self, ImageFormat},
    registry::{self, Family, Frontend, Operation, Platform, Request},
};
const SOURCE: &[u8] = include_bytes!("msp-fixtures/raw-9.msp");
#[test]
fn msp_facade_has_exact_opaque_pixels_and_explicit_metadata_loss_and_budget() {
    assert_eq!(ImageFormat::sniff(SOURCE), Some(ImageFormat::Msp));
    let decoded = image::decode(SOURCE).unwrap();
    assert_eq!((decoded.width, decoded.height), (9, 3));
    assert_eq!(decoded.losses, ["msp-metadata-ignored"]);
    assert_eq!(&decoded.rgba[..4], &[255; 4]);
    assert_eq!(&decoded.rgba[4..8], &[0, 0, 0, 255]);
    assert_eq!(
        image::decode_with_pixel_limit(SOURCE, 26).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let encoded = image::encode(ImageFormat::Msp, 9, 3, &decoded.rgba, 90).unwrap();
    assert_eq!(&encoded[..4], b"DanM");
    assert_eq!(image::decode(&encoded).unwrap().rgba, decoded.rgba);
    assert!(image::KNOWN_IMPORT_LOSSES.contains(&"msp-metadata-ignored"));
}
#[test]
fn msp_flat_export_refuses_gray_colors_and_transparency_even_with_other_loss_acknowledgements() {
    for color in [Rgba([255; 4]), Rgba([0, 0, 0, 255])] {
        let doc = new_document(9, 3, Some(color), 0, POLICY).unwrap();
        let snapshot = doc.clone();
        let exported = export::export_flat(&doc, ImageFormat::Msp, 90, &[]).unwrap();
        assert!(exported.losses.is_empty());
        assert!(doc.same_semantics(&snapshot));
        assert_eq!(&image::decode(&exported.bytes).unwrap().rgba[..4], &color.0);
    }
    for color in [
        Rgba([1, 2, 3, 255]),
        Rgba([128, 128, 128, 255]),
        Rgba([0; 4]),
    ] {
        let doc = new_document(9, 3, Some(color), 0, POLICY).unwrap();
        assert_eq!(
            export::export_flat(
                &doc,
                ImageFormat::Msp,
                90,
                &export::KNOWN_LOSSES.map(str::to_owned)
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
    }
}
#[test]
fn msp_profile_allows_explicit_extended_import_and_fresh_studio_export_only() {
    for platform in [Platform::Native, Platform::Web] {
        for frontend in [
            Frontend::Studio,
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            assert_eq!(
                registry::admit_import(frontend, "legacy.msp", SOURCE, platform).unwrap(),
                Family::Msp
            );
            assert!(registry::admit_open(frontend, "legacy.msp", SOURCE, platform).is_err());
            for operation in [Operation::SavePreserving, Operation::SaveAsCopy] {
                assert!(
                    !registry::admission(Request::new(frontend, operation, Family::Msp, platform))
                        .available()
                );
            }
            assert_eq!(
                registry::admission(Request::new(
                    frontend,
                    Operation::ExportFlat,
                    Family::Msp,
                    platform
                ))
                .available(),
                frontend == Frontend::Studio
            );
        }
        assert!(registry::admit_import(Frontend::Studio, "wrong.png", SOURCE, platform).is_err());
    }
}
