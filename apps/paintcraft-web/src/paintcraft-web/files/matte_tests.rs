use super::*;

#[test]
fn browser_encoder_delivers_matte_without_changing_project_or_legacy_white() {
    let rgba: Vec<u8> = (0..64).flat_map(|_| [17, 92, 231, 128]).collect();
    let document = paintcraft_document::import_image(8, 8, &rgba, "web-jpeg-matte").unwrap();
    let before = document.clone();
    for frontend in [
        Frontend::ClassicV11,
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::Studio,
    ] {
        let default = flat_export::encode(
            &document,
            Some(ImageExport::Jpeg),
            90,
            frontend,
            Default::default(),
            None,
            Default::default(),
        )
        .unwrap();
        let white = flat_export::encode(
            &document,
            Some(ImageExport::Jpeg),
            90,
            frontend,
            Default::default(),
            Some(image::JpegMatte::WHITE),
            Default::default(),
        )
        .unwrap();
        assert_eq!(default, white);
        let bytes = flat_export::encode(
            &document,
            Some(ImageExport::Jpeg),
            90,
            frontend,
            Default::default(),
            Some(image::JpegMatte::new([32, 160, 224])),
            Default::default(),
        )
        .unwrap();
        assert_ne!(bytes, white);
        let decoded = image::decode(&bytes).unwrap();
        assert!(decoded.rgba.as_chunks::<4>().0.iter().all(|pixel| {
            pixel[..3]
                .iter()
                .zip([24u8, 126, 228])
                .all(|(a, b)| a.abs_diff(b) <= 2)
        }));
        assert!(document.same_semantics(&before));
        for format in [
            None,
            Some(ImageExport::Png),
            Some(ImageExport::Pcx),
            Some(ImageExport::Dds),
        ] {
            assert_eq!(
                flat_export::encode(
                    &document,
                    format,
                    90,
                    frontend,
                    Default::default(),
                    Some(image::JpegMatte::WHITE),
                    Default::default(),
                )
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
}
