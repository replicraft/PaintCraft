//! Proposed Vibrance cold archive/spill regression; not registered or executed.
use super::*;
use paintcraft_core::types::{
    Rect,
    filters::{ExposureSrgbV1, LevelsRgbV1, VibranceRgbV1},
    resources::SharedBudget,
};
use paintcraft_document::{
    FilterMask,
    source_metadata::{
        BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
        SourceMetadata,
    },
};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};

fn descriptor(amount: i16) -> Descriptor {
    Descriptor::VibranceRgbV1(VibranceRgbV1::new(amount).unwrap())
}

#[test]
fn vibrance_two_byte_intent_and_unique_payloads_survive_actual_cold_spill_navigation() {
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
            FilterInstance::new(Uuid::from_u128(1), false, descriptor(0))
                .with_mask(Some(FilterMask::new(false, field.clone()))),
            FilterInstance::new(Uuid::from_u128(2), true, descriptor(-100))
                .with_mask(Some(FilterMask::new(true, field))),
        ])
        .unwrap();
        let mut mixed = first.layers[&layer].filters.as_slice().to_vec();
        mixed.push(FilterInstance::new(
            Uuid::from_u128(3),
            false,
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 333, 240, 16).unwrap()),
        ));
        mixed.push(FilterInstance::new(
            Uuid::from_u128(4),
            true,
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(-100).unwrap()),
        ));
        first.layers.get_mut(&layer).unwrap().filters = FilterStack::new(mixed).unwrap();
        first.state_id = Uuid::from_u128(1901);
        let first = Arc::new(first);
        let weak = Arc::downgrade(&first);
        let expected_digest = first.layers[&layer].filters.digest();
        let mut second = (*first).clone();
        let entries = second.layers[&layer].filters.as_slice();
        let updated = FilterStack::new(vec![
            entries[0].clone(),
            entries[1].with_descriptor(descriptor(100)),
            entries[2].clone(),
            entries[3].clone(),
        ])
        .unwrap();
        second.layers.get_mut(&layer).unwrap().filters = updated;
        second.state_id = Uuid::from_u128(1902);
        let second = Arc::new(second);
        assert_ne!(second.layers[&layer].filters.digest(), expected_digest);
        assert_eq!(first.layers[&layer].raster, plain.layers[&layer].raster);
        assert_eq!(second.layers[&layer].raster, plain.layers[&layer].raster);
        let versions = |document: &Document| {
            document.layers[&layer]
                .raster
                .tile_leases()
                .map(|(coordinate, tile)| (coordinate, tile.version()))
                .collect::<Vec<_>>()
        };
        let original_versions = versions(&plain);
        assert!(!original_versions.is_empty());
        assert_eq!(versions(&first), original_versions);
        assert_eq!(versions(&second), original_versions);
        let mut history = History::new(snapshot(plain.clone(), 1));
        let original_decoded = history.memory_usage().decoded_bytes;
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
        assert_eq!(
            history.memory_usage().decoded_bytes,
            original_decoded,
            "Raw raster payload remains uniquely shared across typed snapshots"
        );
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
        assert_eq!(restored.document.state_id, Uuid::from_u128(1901));
        let restored_versions = versions(&restored.document);
        assert_eq!(
            restored_versions
                .iter()
                .map(|(coordinate, _)| *coordinate)
                .collect::<Vec<_>>(),
            original_versions
                .iter()
                .map(|(coordinate, _)| *coordinate)
                .collect::<Vec<_>>()
        );
        // TileVersion is a weak resident allocation identity, not an archived
        // content version. Spilling and dropping all source owners requires
        // fresh identities; keeping the old weak tokens prevents address reuse.
        for ((_, restored), (_, original)) in restored_versions.iter().zip(&original_versions) {
            assert_ne!(restored, original);
        }
        assert_eq!(
            (
                restored.document.layers[&layer].raster.width(),
                restored.document.layers[&layer].raster.height()
            ),
            (2, 1)
        );
        let stack = &restored.document.layers[&layer].filters;
        assert_eq!(stack.digest(), expected_digest);
        assert_eq!(stack.as_slice()[0].descriptor(), descriptor(0));
        assert!(!stack.as_slice()[0].enabled());
        assert_eq!(stack.as_slice()[1].descriptor(), descriptor(-100));
        assert_eq!(
            stack.as_slice()[2].descriptor(),
            Descriptor::LevelsRgbV1(LevelsRgbV1::new(0, 255, 333, 240, 16).unwrap())
        );
        assert!(!stack.as_slice()[2].enabled());
        assert_eq!(
            stack.as_slice()[3].descriptor(),
            Descriptor::ExposureSrgbV1(ExposureSrgbV1::new(-100).unwrap())
        );
        assert!(stack.as_slice()[3].enabled());
        assert_eq!(stack.len(), 4);
        for (index, expected) in [[0u8, 0u8], [255, 156]].into_iter().enumerate() {
            let (bytes, length) = stack.as_slice()[index]
                .descriptor()
                .semantic_parameter_bytes();
            assert_eq!(length, 2);
            assert_eq!(&bytes[..length], &expected);
        }
        for entry in stack.iter().take(2) {
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
        assert_eq!(changed.document.state_id, Uuid::from_u128(1902));
        assert_eq!(
            changed.document.layers[&layer].filters.as_slice()[1].descriptor(),
            descriptor(100)
        );
        assert_ne!(
            changed.document.layers[&layer].filters.digest(),
            expected_digest
        );
        assert_eq!(
            changed.document.layers[&layer].filters.as_slice()[0],
            stack.as_slice()[0]
        );
        assert_eq!(
            changed.document.layers[&layer].filters.as_slice()[2],
            stack.as_slice()[2]
        );
        assert_eq!(
            changed.document.layers[&layer].filters.as_slice()[3],
            stack.as_slice()[3]
        );
        // These live restored snapshots share the same decoded blob payload.
        // Only their typed filter intention differs.
        assert_eq!(versions(&again.document), restored_versions);
        assert_eq!(versions(&changed.document), restored_versions);
        assert_eq!(history.memory_usage().filter_mask_bytes, mask_usage.1);
        assert_eq!(
            history.memory_usage().source_metadata_bytes,
            metadata_usage.1
        );
        assert_eq!(
            changed.document.layers[&layer].filters.as_slice()[1].mask(),
            stack.as_slice()[1].mask()
        );
    }
    assert_eq!(coverage_budget.usage().reserved, 0);
    assert_eq!(metadata_budget.usage().reserved, 0);
}
