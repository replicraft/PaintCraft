use super::*;
use paintcraft_core::types::LayerBlendMode;
#[test]
fn blend_modes_survive_real_cold_spill_seek_and_release_with_masks_and_hidden_rgb() {
    let budget = SharedBudget::new(8192);
    let metadata_budget = SharedBudget::new(8192);
    let field = coverage(&budget);
    let usage = field.storage_usage().unwrap();
    let first = masked(base(), field.clone(), 11);
    let id = first.roots[0];
    let mut first = first;
    first.layers.get_mut(&id).unwrap().blend_mode = LayerBlendMode::Multiply;
    first.layers.get_mut(&id).unwrap().mask =
        Some(paintcraft_document::LayerMask::new(false, field.clone()));
    first.source_metadata = Some(super::layer_tests::attachment(&metadata_budget));
    let metadata_digest = first.source_metadata.as_ref().unwrap().digest();
    let first = Arc::new(first);
    let weak = Arc::downgrade(&first);
    let mut second = (*first).clone();
    second.state_id = Uuid::from_u128(12);
    second.layers.get_mut(&id).unwrap().blend_mode = LayerBlendMode::Screen;
    let mut history = History::new(snapshot(base(), 1));
    history
        .record(Snapshot {
            document: first.clone(),
            selection: Selection::Inactive,
            history_id: Some(Uuid::from_u128(2)),
        })
        .unwrap();
    history.record(snapshot(second, 3)).unwrap();
    history.record(snapshot(base(), 4)).unwrap();
    history.compact(1, 4 << 20, || Ok(())).unwrap();
    drop(first);
    drop(field);
    assert!(weak.upgrade().is_none());
    let store = Arc::new(Disk::new());
    history.set_spill_store(store.clone());
    assert!(history.spill(4 << 20, 16, || Ok(())).unwrap().payloads > 0);
    for (target, mode) in [
        (2, LayerBlendMode::Multiply),
        (3, LayerBlendMode::Screen),
        (2, LayerBlendMode::Multiply),
    ] {
        let restored = history
            .seek(Some(Uuid::from_u128(target)))
            .unwrap()
            .unwrap()
            .0;
        let layer = &restored.document.layers[&id];
        assert_eq!(layer.blend_mode, mode);
        assert_eq!(
            restored.document.source_metadata.as_ref().unwrap().digest(),
            metadata_digest
        );
        assert_eq!(layer.raster.get(0, 0), Rgba([13, 71, 201, 0]));
        assert_eq!(layer.raster.get(1, 0), Rgba([250, 1, 20, 255]));
        assert_eq!(layer.filters.as_slice().len(), 2);
        assert!(!layer.mask.as_ref().unwrap().enabled());
        assert_eq!(
            layer.mask.as_ref().unwrap().coverage().storage_usage(),
            Some(usage)
        );
    }
    drop(history);
    drop(store);
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(metadata_budget.usage().reserved, 0);
}
