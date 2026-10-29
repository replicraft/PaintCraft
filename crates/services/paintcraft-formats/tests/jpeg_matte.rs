use paintcraft_core::types::ErrorCode;
use paintcraft_formats::{
    export,
    image::{ImageFormat, JpegMatte},
};

#[test]
fn explicit_non_jpeg_matte_fails_before_render_and_jpeg_keeps_its_exact_loss_contract() {
    let document =
        paintcraft_document::new_document(8, 8, None, 0, paintcraft_document::POLICY).unwrap();
    for format in [
        ImageFormat::Png,
        ImageFormat::Bmp,
        ImageFormat::Gif,
        ImageFormat::Tiff,
        ImageFormat::Pcx,
        ImageFormat::Msp,
        ImageFormat::Dds,
    ] {
        let mut options = export::FlatOptions::new(format, 90);
        options.jpeg_matte = Some(JpegMatte::WHITE);
        let error = export::export_flat_options_with(&document, options, &[], |_| {
            panic!("non-JPEG matte must fail before render")
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InputSchema);
    }
    let mut options = export::FlatOptions::new(ImageFormat::Jpeg, 90);
    options.jpeg_matte = Some(JpegMatte::new([32, 160, 224]));
    let mut rgba = Vec::new();
    for _ in 0..64 {
        rgba.extend_from_slice(&[17, 92, 231, 128]);
    }
    let before = document.clone();
    assert_eq!(
        export::export_flat_options_with(&document, options, &[], |_| Ok(rgba.clone()))
            .unwrap_err()
            .code,
        ErrorCode::CapabilityUnsupported
    );
    let allowed = vec!["lossy-jpeg".into(), "alpha-discarded".into()];
    let result =
        export::export_flat_options_with(&document, options, &allowed, |_| Ok(rgba)).unwrap();
    assert_eq!(result.losses, allowed);
    let decoded = paintcraft_formats::image::decode(&result.bytes).unwrap();
    // Authored rational oracle: source[17,92,231] at alpha128 over[32,160,224].
    for pixel in decoded.rgba.as_chunks::<4>().0 {
        assert!(
            pixel[..3]
                .iter()
                .zip([24u8, 126, 228])
                .all(|(actual, expected)| actual.abs_diff(expected) <= 2)
        );
        assert_eq!(pixel[3], 255);
    }
    assert!(document.same_semantics(&before));
}
