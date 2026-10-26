use super::*;
use paintcraft_formats::jxl::{self, Samples};

#[test]
fn browser_studio_single_pixel_jxl_export_is_exact_and_classic_remains_denied() {
    let pixel = [17, 29, 91, 255];
    let doc = paintcraft_document::import_image(1, 1, &pixel, &"0".repeat(64)).unwrap();
    let before = doc.clone();
    let bytes =
        encode_quality_for_profile(&doc, Some(ImageExport::Jxl), 90, Frontend::Studio).unwrap();
    let decoded = jxl::decode_with_pixel_limit(&bytes, 1).unwrap();
    assert_eq!((decoded.width, decoded.height), (1, 1));
    assert_eq!(decoded.samples, Samples::Rgba8(pixel.to_vec()));
    assert!(doc.same_semantics(&before));
    assert_eq!(doc.state_id, before.state_id);
    assert!(
        encode_quality_for_profile(&doc, Some(ImageExport::Jxl), 90, Frontend::ClassicV11).is_err()
    );
}
