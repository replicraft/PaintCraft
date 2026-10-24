use paintcraft_core::types::{ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::{image, pcnd, png};

#[test]
fn constrained_image_decode_rejects_before_materialization() {
    let encoded = png::encode_rgba8(16, 16, &[255; 16 * 16 * 4]).unwrap();
    assert_eq!(
        image::decode_with_pixel_limit(&encoded, 255)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let decoded = image::decode_with_pixel_limit(&encoded, 256).unwrap();
    assert_eq!(decoded.rgba, vec![255; 16 * 16 * 4]);
    assert_eq!(image::decode(&encoded).unwrap().rgba, decoded.rgba);
}

#[test]
fn constrained_pcnd_checks_canvas_even_without_allocated_tiles() {
    let doc = new_document(4096, 4096, Some(Rgba([255; 4])), 1, POLICY).unwrap();
    let bytes = pcnd::write(&doc).unwrap();
    assert_eq!(
        pcnd::read_with_limits(&bytes, 4_194_304, 0)
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert!(pcnd::read(&bytes).unwrap().same_semantics(&doc));
    let small = new_document(16, 16, Some(Rgba([255; 4])), 1, POLICY).unwrap();
    assert!(
        pcnd::read_with_limits(&pcnd::write(&small).unwrap(), 256, 0)
            .unwrap()
            .same_semantics(&small)
    );
}

#[test]
fn constrained_pcnd_rejects_tile_staging_without_weakening_default_reader() {
    let doc = paintcraft_document::import_image(2, 2, &[10; 16], "host-limit").unwrap();
    let bytes = pcnd::write(&doc).unwrap();
    assert_eq!(
        pcnd::read_with_limits(&bytes, 4, 0).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert!(pcnd::read(&bytes).unwrap().same_semantics(&doc));
}
