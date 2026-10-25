use paintcraft_core::types::{ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_formats::{
    export,
    image::{self, ImageFormat},
};

#[test]
fn gif_conversion_requires_exact_loss_consent_and_tiff_keeps_rgba_samples() {
    let doc = new_document(3, 2, Some(Rgba([17, 59, 83, 128])), 0, POLICY).unwrap();
    let before = doc.clone();
    let denied = export::export_flat(&doc, ImageFormat::Gif, 90, &[]).unwrap_err();
    assert_eq!(denied.code, ErrorCode::CapabilityUnsupported);
    assert!(denied.message.contains("palette-quantized"));
    assert!(denied.message.contains("alpha-binarized"));
    let allowed = vec!["palette-quantized".into(), "alpha-binarized".into()];
    let gif = export::export_flat(&doc, ImageFormat::Gif, 90, &allowed).unwrap();
    assert_eq!(gif.losses, allowed);
    let decoded = image::decode(&gif.bytes).unwrap();
    assert!(
        decoded
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 255)
    );
    let tiff = export::export_flat(&doc, ImageFormat::Tiff, 90, &[]).unwrap();
    assert!(tiff.losses.is_empty());
    let decoded = image::decode(&tiff.bytes).unwrap();
    assert_eq!(decoded.rgba, paintcraft_render::render_flat(&doc).unwrap());
    assert!(doc.same_semantics(&before));
}

#[test]
fn binary_transparency_is_not_reported_as_lost_alpha() {
    let doc = new_document(2, 3, Some(Rgba::TRANSPARENT), 0, POLICY).unwrap();
    let gif =
        export::export_flat(&doc, ImageFormat::Gif, 90, &["palette-quantized".into()]).unwrap();
    assert_eq!(gif.losses, ["palette-quantized"]);
    assert!(
        image::decode(&gif.bytes)
            .unwrap()
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] == 0)
    );
}
