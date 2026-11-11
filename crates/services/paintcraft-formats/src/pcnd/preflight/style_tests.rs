use super::*;
use paintcraft_core::types::styles::{ColorOverlayRgbV1, EffectBlendMode};
use paintcraft_document::{
    StyleDescriptor, StyleInstance, StyleStack,
    source_metadata::{BYTES_MAX, SourceMetadata},
};
fn styled() -> Document {
    let mut doc = paintcraft_document::new_document(
        1,
        1,
        Some(Rgba([20, 80, 200, 127])),
        0,
        paintcraft_document::POLICY,
    )
    .unwrap();
    let layer = doc.layers.get_mut(&doc.roots[0]).unwrap();
    layer.name = "Layer".into();
    layer.raster = RasterLayer::new(1, 1, Rgba([20, 80, 200, 127]));
    layer.styles = StyleStack::new(vec![
        StyleInstance::new(
            Uuid::from_u128(0x14),
            true,
            StyleDescriptor::ColorOverlayRgbV1(ColorOverlayRgbV1::new(
                [220, 40, 0],
                128,
                EffectBlendMode::Normal,
            )),
        )
        .unwrap(),
    ])
    .unwrap();
    doc
}
#[test]
fn styled_normal_forced_v9_fast_framing_matches_independent108_before_source_profile() {
    let doc = styled();
    // Frozen single PCND9 specimen:108 metadata bytes, no raster records.
    assert_eq!(coarse_bound(&doc, &|| Ok(())).unwrap(), Some(108));
    assert_eq!(upper_bound(&doc, &|| Ok(())).unwrap(), Some(108));
    assert_eq!(write(&doc).unwrap().len(), 108 + 32);
    assert_eq!(write_compact(&doc).unwrap().len(), 108 + 32);
}
#[test]
fn styled_fast_preflight_and_actual_writer_agree_exact8m_and_one_more_byte() {
    let mut template = read(include_bytes!(
        "../../../tests/pcnd-metadata-fixtures/rgb-profile.pcnd"
    ))
    .unwrap();
    let original = template.source_metadata.take().unwrap();
    let descriptor = *original.descriptor();
    // Independent framing108 + existing authenticated source header192 =300.
    let exact_profile = DOCUMENT_MAX as usize - 300;
    for extra in [0, 1] {
        let mut profile = original.profile().unwrap().to_vec();
        profile.resize(exact_profile + extra, 0);
        let length = profile.len() as u32;
        profile[..4].copy_from_slice(&length.to_be_bytes());
        let realm = SharedBudget::new(BYTES_MAX + 4096);
        let source =
            SourceMetadata::capture(descriptor, Some(&profile), BYTES_MAX, &realm, &|| Ok(()))
                .unwrap();
        let doc = styled()
            .with_source_metadata_checked(Some(source), &|| Ok(()))
            .unwrap();
        assert_eq!(
            coarse_bound(&doc, &|| Ok(())).unwrap(),
            Some(DOCUMENT_MAX + extra as u64)
        );
        assert_eq!(
            upper_bound(&doc, &|| Ok(())).unwrap(),
            Some(DOCUMENT_MAX + extra as u64)
        );
        assert_eq!(
            check_metadata_admission(&doc, &|| Ok(())).is_ok(),
            extra == 0
        );
        assert_eq!(
            check_compact_metadata_admission(&doc, &|| Ok(())).is_ok(),
            extra == 0
        );
        let raw = write(&doc);
        let compact = write_compact(&doc);
        if extra == 0 {
            assert_eq!(raw.unwrap().len(), DOCUMENT_MAX as usize + 32);
            assert_eq!(compact.unwrap().len(), DOCUMENT_MAX as usize + 32);
        } else {
            assert_eq!(raw.unwrap_err().code, ErrorCode::ResourceLimit);
            assert_eq!(compact.unwrap_err().code, ErrorCode::ResourceLimit);
        }
        drop(doc);
        assert_eq!(realm.usage().reserved, 0);
    }
}
