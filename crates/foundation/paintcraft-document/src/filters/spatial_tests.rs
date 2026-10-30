use super::*;
use paintcraft_core::types::{
    Rect, Rgba,
    filters::{BlackWhiteV1, BoxBlurV1, BrightnessContrastV1, PosterizeRgbV1, ThresholdLumaV1},
    resources::SharedBudget,
};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};

fn stack(descriptor: Descriptor) -> FilterStack {
    FilterStack::new(vec![FilterInstance::new(Uuid::nil(), true, descriptor)]).unwrap()
}

#[test]
fn six_frozen_point_hashes_and_independent_box_framing_remain_exact() {
    // Independent Python hashlib over domain, count, nil UUID, enabled byte,
    // descriptor ID length/UTF8 and the descriptor's declared parameter bytes.
    for (descriptor, expected) in [
        (
            Descriptor::InvertRgbV1,
            "3cfb2108a9b81c1d46bf9f8d260801ee2a36d878215bea720dc88da74cc6da2b",
        ),
        (
            Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-19, 23).unwrap()),
            "7282596b7c38dba714b1e5702b33dc1e0d6a789be5524e362fe1747c687d261f",
        ),
        (
            Descriptor::BlackWhiteRgbV1(BlackWhiteV1::new(50).unwrap()),
            "650ec8b3025aff32882c782b71688d44fa0266d5718a57f8e0af488baba60200",
        ),
        (
            Descriptor::ThresholdLumaV1(ThresholdLumaV1::new(128).unwrap()),
            "6442826d99e0f9fe765853008b061a49f1de38fbb114a787f902c3f045b5165a",
        ),
        (
            Descriptor::PosterizeRgbV1(PosterizeRgbV1::new(6).unwrap()),
            "4ce37fb66494a94506cce4d555f15f7e195f663559582cb632366eb297c2903a",
        ),
        (
            Descriptor::SolarizeRgbV1,
            "ca35efcca45782b936ab08bb2ac4383a2a0c64e5841ae1b54130157212d456cd",
        ),
        (
            Descriptor::BoxBlurV1(BoxBlurV1::new(2).unwrap()),
            "1800db11bcd082adca7d3ca728619b29f78c14a03c2a1a5e194d39f635605880",
        ),
    ] {
        assert_eq!(crate::hex(&stack(descriptor).digest()), expected);
    }
    let mut seen = std::collections::HashSet::new();
    for radius in 0..=32 {
        assert!(
            seen.insert(stack(Descriptor::BoxBlurV1(BoxBlurV1::new(radius).unwrap())).digest())
        );
    }
    assert!(!seen.contains(&stack(Descriptor::InvertRgbV1).digest()));
}

#[test]
fn box_edits_keep_mask_ownership_source_metadata_and_raw_raster_unchanged() {
    use crate::source_metadata::{
        BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
        SourceMetadata,
    };
    let coverage_budget = SharedBudget::new(8192);
    let metadata_budget = SharedBudget::new(8192);
    {
        let coverage = A8Mask::uniform(
            Rect::new(-1, -1, 4, 4).unwrap(),
            127,
            &coverage_budget,
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        let original = FilterInstance::new(Uuid::from_u128(8), false, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(false, coverage)));
        let mask_usage = original.mask().unwrap().coverage().storage_usage();
        let first = original.with_descriptor(Descriptor::BoxBlurV1(BoxBlurV1::new(0).unwrap()));
        let second = first.with_descriptor(Descriptor::BoxBlurV1(BoxBlurV1::new(32).unwrap()));
        let duplicate = second.with_id(Uuid::from_u128(9)).with_enabled(true);
        for instance in [&first, &second, &duplicate] {
            assert_eq!(instance.mask(), original.mask());
            assert_eq!(
                instance.mask().unwrap().coverage().storage_usage(),
                mask_usage
            );
        }
        assert_eq!(first.id(), original.id());
        assert!(!first.enabled());
        assert_ne!(
            stack(first.descriptor()).digest(),
            stack(second.descriptor()).digest()
        );
        let mut before =
            crate::new_document(2, 2, Some(Rgba([17, 30, 90, 0])), 23, crate::POLICY).unwrap();
        before.source_metadata = Some(
            SourceMetadata::capture(
                SourceDescriptor {
                    format: SourceFormat::Png,
                    source_model: SourceColorModel::Rgb,
                    source_sha256: [3; 32],
                    source_bytes: 100,
                    selection: None,
                    source_dimensions: (2, 2),
                    output_dimensions: (2, 2),
                    orientation: None,
                    orientation_policy: OrientationPolicy::Unmodified,
                    resolution: None,
                },
                None,
                BYTES_MAX,
                &metadata_budget,
                &|| Ok(()),
            )
            .unwrap(),
        );
        before.layers.get_mut(&before.roots[0]).unwrap().filters =
            FilterStack::new(vec![original.clone()]).unwrap();
        let mut after = before.clone();
        after.layers.get_mut(&after.roots[0]).unwrap().filters =
            FilterStack::new(vec![second.clone(), duplicate]).unwrap();
        assert!(!after.same_semantics(&before));
        assert_eq!(after.source_metadata, before.source_metadata);
        assert_eq!(
            after.source_metadata.as_ref().unwrap().storage_usage(),
            before.source_metadata.as_ref().unwrap().storage_usage()
        );
        assert_eq!(
            after.layers[&after.roots[0]].raster,
            before.layers[&before.roots[0]].raster
        );
        assert_eq!(
            after.layers[&after.roots[0]].raster.get(0, 0),
            Rgba([17, 30, 90, 0])
        );
        assert_eq!(coverage_budget.usage().reserved, mask_usage.unwrap().1);
        let summary = inspection(&after.layers[&after.roots[0]].filters);
        assert_eq!(
            summary.as_array().unwrap()[0].get("descriptor"),
            Some(&second.descriptor().to_value())
        );
        assert_eq!(
            summary.as_array().unwrap()[0].get("mask"),
            inspection(&before.layers[&before.roots[0]].filters)
                .as_array()
                .unwrap()[0]
                .get("mask")
        );
    }
    assert_eq!(coverage_budget.usage().reserved, 0);
    assert_eq!(metadata_budget.usage().reserved, 0);
}
