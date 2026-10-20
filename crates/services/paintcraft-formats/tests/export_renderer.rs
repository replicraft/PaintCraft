use paintcraft_core::types::{ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::{export::export_flat_with, image::ImageFormat};

#[test]
fn exact_renderer_export_rejects_unknown_losses_before_work_and_malformed_output_before_encoding() {
    let doc = new_document(3, 2, Some(Rgba([1, 2, 3, 255])), 0, POLICY).unwrap();
    let bad = vec!["not-a-loss".to_owned()];
    let error = export_flat_with(&doc, ImageFormat::Png, 90, &bad, |_| {
        panic!("renderer must not run")
    })
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::InputSchema);
    for bytes in [vec![], vec![0; 23], vec![0; 25]] {
        let error = export_flat_with(&doc, ImageFormat::Png, 90, &[], |_| Ok(bytes)).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvariantViolation);
    }
}
