//! Host export wiring is distinct from qualification of file decoding.
use super::*;
use paintcraft_core::types::Rgba;
use paintcraft_formats::{dds, jxl};

fn document() -> Document {
    paintcraft_document::new_document(
        4,
        3,
        Some(Rgba([17, 51, 93, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap()
}
#[test]
fn studio_specialist_exports_retain_the_source_and_classic_cannot_select_them() {
    let document = document();
    let expected = paintcraft_render::render_flat(&document).unwrap();
    for format in [ImageExport::Jxl, ImageExport::Dds, ImageExport::Avif] {
        for profile in [
            Frontend::ClassicV7,
            Frontend::ClassicV10,
            Frontend::ClassicV11,
        ] {
            assert!(encode_quality_for_profile(&document, Some(format), 90, profile).is_err());
        }
        let bytes =
            encode_quality_for_profile(&document, Some(format), 90, Frontend::Studio).unwrap();
        match format {
            ImageExport::Jxl => {
                assert!(jxl::sniff(&bytes));
                let decoded = jxl::decode_with_pixel_limit(&bytes, 12).unwrap();
                assert_eq!(decoded.samples, jxl::Samples::Rgba8(expected.clone()));
                assert!(
                    decode_input(&bytes, "copy.jxl".into(), Frontend::Studio, false).is_err(),
                    "internal decoder does not qualify general Open"
                );
            }
            ImageExport::Dds => {
                let decoded = dds::decode_with_pixel_limit(&bytes, 12).unwrap();
                assert_eq!(decoded.rgba, expected);
                assert_eq!(decoded.info.encoding, dds::Encoding::Rgba8);
                assert_eq!(decoded.info.transfer, dds::Transfer::Srgb);
                assert_eq!(decoded.info.alpha, dds::SourceAlpha::Straight);
            }
            ImageExport::Avif => {
                assert_eq!(bytes.get(4..8), Some(b"ftyp".as_slice()));
                assert!(decode_input(&bytes, "copy.avif".into(), Frontend::Studio, false).is_err());
                assert_eq!(format.mime(), "image/avif");
            }
            _ => unreachable!(),
        }
        assert_eq!(paintcraft_render::render_flat(&document).unwrap(), expected);
    }
}
#[test]
fn avif_alpha_and_color_losses_are_explicit_and_quality_100_is_still_lossy() {
    let doc = document();
    assert!(
        paintcraft_formats::export::export_flat(&doc, image::ImageFormat::Avif, 100, &[]).is_err()
    );
    let result = paintcraft_formats::export::export_flat(
        &doc,
        image::ImageFormat::Avif,
        100,
        &["lossy-avif".into(), "alpha-lossy".into()],
    )
    .unwrap();
    assert_eq!(result.losses, ["lossy-avif", "alpha-lossy"]);
    let low =
        encode_quality_for_profile(&doc, Some(ImageExport::Avif), 10, Frontend::Studio).unwrap();
    let high =
        encode_quality_for_profile(&doc, Some(ImageExport::Avif), 100, Frontend::Studio).unwrap();
    assert_ne!(low, high);
}
