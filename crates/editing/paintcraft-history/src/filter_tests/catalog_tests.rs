//! Cold history retains typed catalog intent, not JSON or an invented codec.
use super::*;
use paintcraft_core::types::{Rect, filters::LevelsRgbV1, resources::SharedBudget};
use paintcraft_document::{
    FilterMask,
    source_metadata::{
        BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
        SourceMetadata,
    },
};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};

fn descriptor(gamma: i16, output_black: i16, output_white: i16) -> Descriptor {
    Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, gamma, output_black, output_white).unwrap())
}

#[test]
fn catalog_intent_masks_and_original_metadata_survive_real_cold_spill_navigation() {
    let coverage_budget = SharedBudget::new(8192);
    let metadata_budget = SharedBudget::new(8192);
    {
        let plain = Arc::new(document());
        let layer = plain.roots[0];
        let field = A8Mask::uniform(
            Rect::new(-1, -1, 4, 3).unwrap(),
            127,
            &coverage_budget,
            Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        let mask_usage = field.storage_usage().unwrap();
        let mask_digest = field.digest();
        let mut first = (*plain).clone();
        first.source_metadata = Some(
            SourceMetadata::capture(
                SourceDescriptor {
                    format: SourceFormat::Png,
                    source_model: SourceColorModel::Rgb,
                    source_sha256: [3; 32],
                    source_bytes: 100,
                    selection: None,
                    source_dimensions: (2, 1),
                    output_dimensions: (2, 1),
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
        let metadata_digest = first.source_metadata.as_ref().unwrap().digest();
        let metadata_usage = first.source_metadata.as_ref().unwrap().storage_usage();
        first.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![
            FilterInstance::new(Uuid::from_u128(1), false, descriptor(100, 0, 255))
                .with_mask(Some(FilterMask::new(false, field.clone()))),
            FilterInstance::new(Uuid::from_u128(2), true, descriptor(999, 240, 16))
                .with_mask(Some(FilterMask::new(true, field))),
        ])
        .unwrap();
        first.state_id = Uuid::from_u128(801);
        let first = Arc::new(first);
        let weak = Arc::downgrade(&first);
        let expected_digest = first.layers[&layer].filters.digest();
        let mut second = (*first).clone();
        let entries = second.layers[&layer].filters.as_slice();
        let updated = FilterStack::new(vec![
            entries[0].clone(),
            entries[1].with_descriptor(descriptor(10, 73, 73)),
        ])
        .unwrap();
        second.layers.get_mut(&layer).unwrap().filters = updated;
        second.state_id = Uuid::from_u128(802);
        let second = Arc::new(second);
        assert_ne!(second.layers[&layer].filters.digest(), expected_digest);
        assert_eq!(first.layers[&layer].raster, plain.layers[&layer].raster);
        assert_eq!(second.layers[&layer].raster, plain.layers[&layer].raster);
        let mut history = History::new(snapshot(plain.clone(), 1));
        history.record(snapshot(first.clone(), 2)).unwrap();
        history.record(snapshot(second.clone(), 3)).unwrap();
        history
            .record(snapshot(
                Arc::new(
                    paintcraft_document::new_document(2, 1, None, 0, paintcraft_document::POLICY)
                        .unwrap(),
                ),
                4,
            ))
            .unwrap();
        assert_eq!(history.memory_usage().filter_mask_bytes, mask_usage.1);
        assert_eq!(
            history.memory_usage().source_metadata_bytes,
            metadata_usage.1
        );
        assert_eq!(
            history
                .compact(1, 4 << 20, || Ok(()))
                .unwrap()
                .packed_entries,
            3
        );
        drop((plain, first, second));
        assert!(
            weak.upgrade().is_none(),
            "Actual cold reconstruction must be required"
        );
        let store = Arc::new(Store::default());
        history.set_spill_store(store.clone());
        assert!(history.spill(4 << 20, 16, || Ok(())).unwrap().payloads > 0);
        assert!(store.usage().reserved_bytes > 0);
        assert_eq!(coverage_budget.usage().reserved, mask_usage.1);
        assert_eq!(metadata_budget.usage().reserved, metadata_usage.1);
        let restored = history.seek(Some(Uuid::from_u128(2))).unwrap().unwrap().0;
        assert_eq!(restored.document.state_id, Uuid::from_u128(801));
        let stack = &restored.document.layers[&layer].filters;
        assert_eq!(stack.digest(), expected_digest);
        assert_eq!(stack.as_slice()[0].descriptor(), descriptor(100, 0, 255));
        assert!(!stack.as_slice()[0].enabled());
        assert_eq!(stack.as_slice()[1].descriptor(), descriptor(999, 240, 16));
        for entry in stack.iter() {
            assert_eq!(entry.mask().unwrap().coverage().digest(), mask_digest);
            assert_eq!(
                entry.mask().unwrap().coverage().storage_usage(),
                Some(mask_usage)
            );
        }
        assert_eq!(
            restored.document.source_metadata.as_ref().unwrap().digest(),
            metadata_digest
        );
        assert_eq!(
            restored
                .document
                .source_metadata
                .as_ref()
                .unwrap()
                .storage_usage(),
            metadata_usage
        );
        assert_eq!(
            restored.document.layers[&layer].raster.get(0, 0),
            Rgba([13, 71, 201, 0])
        );
        assert_eq!(
            restored.document.layers[&layer].raster.get(1, 0),
            Rgba([250, 1, 20, 255])
        );
        let earlier = history.undo().unwrap().unwrap();
        assert!(earlier.document.layers[&layer].filters.is_empty());
        let again = history.redo().unwrap().unwrap();
        assert_eq!(again.document.layers[&layer].filters, *stack);
        let changed = history.redo().unwrap().unwrap();
        assert_eq!(changed.document.state_id, Uuid::from_u128(802));
        assert_eq!(
            changed.document.layers[&layer].filters.as_slice()[1].descriptor(),
            descriptor(10, 73, 73)
        );
        assert_eq!(
            changed.document.layers[&layer].filters.as_slice()[1].mask(),
            stack.as_slice()[1].mask()
        );
    }
    assert_eq!(coverage_budget.usage().reserved, 0);
    assert_eq!(metadata_budget.usage().reserved, 0);
}
