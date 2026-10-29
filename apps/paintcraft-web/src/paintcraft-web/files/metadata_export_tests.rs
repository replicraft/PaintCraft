use super::*;
use paintcraft_formats::export::metadata_policy::Intent;
fn document() -> Document {
    pcnd::read(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    )))
    .unwrap()
}
#[test]
fn browser_actual_encoder_requires_intent_preserves_pcnd_and_refuses_project_omission() {
    let doc = document();
    let project = pcnd::write_compact(&doc).unwrap();
    let before = doc.source_metadata.as_ref().unwrap().digest();
    for frontend in [
        Frontend::ClassicV7,
        Frontend::ClassicV10,
        Frontend::ClassicV11,
        Frontend::Studio,
    ] {
        assert_eq!(
            flat_export::encode(
                &doc,
                Some(ImageExport::Png),
                90,
                frontend,
                Default::default(),
                None,
                Intent::Refuse
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        let bytes = flat_export::encode(
            &doc,
            Some(ImageExport::Png),
            90,
            frontend,
            Default::default(),
            None,
            Intent::Omit,
        )
        .unwrap();
        assert_eq!(
            image::decode(&bytes).unwrap().rgba,
            paintcraft_render::render_flat(&doc).unwrap()
        );
        let preserved = flat_export::encode(
            &doc,
            None,
            90,
            frontend,
            Default::default(),
            None,
            Intent::Refuse,
        )
        .unwrap();
        assert_eq!(preserved, project);
        assert_eq!(
            pcnd::read(&preserved)
                .unwrap()
                .source_metadata
                .as_ref()
                .unwrap()
                .digest(),
            before
        );
        for doc in [
            &doc,
            &paintcraft_document::new_document(1, 1, None, 0, paintcraft_document::POLICY).unwrap(),
        ] {
            assert_eq!(
                flat_export::encode(
                    doc,
                    None,
                    90,
                    frontend,
                    Default::default(),
                    None,
                    Intent::Omit
                )
                .unwrap_err()
                .code,
                ErrorCode::InputSchema
            );
        }
    }
    assert_eq!(pcnd::write_compact(&doc).unwrap(), project);
}
