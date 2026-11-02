//! Real history ownership/spill, not a synthetic serializer round trip.
use super::*;
use paintcraft_document::{
    LayerMask,
    source_metadata::{
        BYTES_MAX, OrientationPolicy, SourceColorModel, SourceDescriptor, SourceFormat,
        SourceMetadata,
    },
};
pub(super) fn attachment(budget: &SharedBudget) -> SourceMetadata {
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
        budget,
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn layer_and_filter_shared_coverage_metadata_and_selection_survive_real_cold_spill_and_undo() {
    let budget = SharedBudget::new(8192);
    let metadata_budget = SharedBudget::new(8192);
    let field = coverage(&budget);
    let usage = field.storage_usage().unwrap();
    let digest = field.digest();
    let plain = base();
    let id = plain.roots[0];
    let versions = plain.layers[&id]
        .raster
        .tile_leases()
        .map(|(c, t)| (c, t.version()))
        .collect::<Vec<_>>();
    assert!(!versions.is_empty());
    let mut first = masked(plain.clone(), field.clone(), 11);
    first.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, field.clone()));
    first.source_metadata = Some(attachment(&metadata_budget));
    let metadata_digest = first.source_metadata.as_ref().unwrap().digest();
    let metadata_bytes = first.source_metadata.as_ref().unwrap().storage_usage().1;
    let first = Arc::new(first);
    let weak = Arc::downgrade(&first);
    let mut second = (*first).clone();
    second.state_id = Uuid::from_u128(12);
    second.layers.get_mut(&id).unwrap().mask =
        Some(first.layers[&id].mask.as_ref().unwrap().with_enabled(true));
    let selection = Selection::Rect(Rect::new(0, 0, 1, 1).unwrap());
    let mut history = History::new(snapshot(plain, 1));
    history
        .record(Snapshot {
            document: first.clone(),
            selection: selection.clone(),
            history_id: Some(Uuid::from_u128(2)),
        })
        .unwrap();
    history.record(snapshot(second, 3)).unwrap();
    history
        .record(snapshot(
            paintcraft_document::new_document(2, 1, None, 9, paintcraft_document::POLICY).unwrap(),
            4,
        ))
        .unwrap();
    assert_eq!(
        history.memory_usage().filter_mask_bytes,
        usage.1,
        "Layer and both filter uses count one backing"
    );
    assert_eq!(history.memory_usage().source_metadata_bytes, metadata_bytes);
    history.compact(1, 4 << 20, || Ok(())).unwrap();
    drop(first);
    drop(field);
    assert!(weak.upgrade().is_none());
    let store = Arc::new(Disk::new());
    let folder = store.folder.0.clone();
    history.set_spill_store(store.clone());
    assert!(history.spill(4 << 20, 16, || Ok(())).unwrap().payloads > 0);
    assert!(store.usage().reserved_bytes > 0);
    assert_eq!(budget.usage().reserved, usage.1);
    assert_eq!(metadata_budget.usage().reserved, metadata_bytes);
    let restored = history.seek(Some(Uuid::from_u128(2))).unwrap().unwrap().0;
    assert_eq!(restored.document.state_id, Uuid::from_u128(11));
    assert_eq!(restored.selection, selection);
    let layer = &restored.document.layers[&id];
    let m = layer.mask.as_ref().unwrap();
    assert!(!m.enabled());
    assert_eq!(m.coverage().digest(), digest);
    assert_eq!(m.coverage().storage_usage(), Some(usage));
    assert!(layer.mask_storage_usage().all(|u| u == usage));
    assert_eq!(layer.raster.get(0, 0), Rgba([13, 71, 201, 0]));
    assert_eq!(layer.raster.get(1, 0), Rgba([250, 1, 20, 255]));
    assert_eq!(
        restored.document.source_metadata.as_ref().unwrap().digest(),
        metadata_digest
    );
    let restored_versions = layer
        .raster
        .tile_leases()
        .map(|(c, t)| (c, t.version()))
        .collect::<Vec<_>>();
    assert_eq!(
        restored_versions.iter().map(|x| x.0).collect::<Vec<_>>(),
        versions.iter().map(|x| x.0).collect::<Vec<_>>()
    );
    for ((_, a), (_, b)) in restored_versions.iter().zip(&versions) {
        assert_ne!(
            a, b,
            "Cold rehydration creates fresh resident payload identity"
        );
    }
    let before = history.undo().unwrap().unwrap();
    assert!(before.document.layers[&id].mask.is_none());
    let redo = history.redo().unwrap().unwrap();
    assert_eq!(redo.document.layers[&id].mask, layer.mask);
    let second = history.redo().unwrap().unwrap();
    assert!(second.document.layers[&id].mask.as_ref().unwrap().enabled());
    assert_eq!(history.memory_usage().filter_mask_bytes, usage.1);
    drop((restored, before, redo, second, history, store));
    assert_eq!(budget.usage().reserved, 0);
    assert_eq!(metadata_budget.usage().reserved, 0);
    assert!(!folder.exists());
}
#[test]
fn independently_equal_layer_backings_count_twice_while_filter_aliases_stay_unique() {
    let budget = SharedBudget::new(8192);
    let a = coverage(&budget);
    let b = coverage(&budget);
    assert_eq!(a, b);
    let bytes = a.storage_usage().unwrap().1;
    assert_ne!(a.storage_usage(), b.storage_usage());
    let mut first = masked(base(), a.clone(), 11);
    let id = first.roots[0];
    first.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, a));
    let mut second = first.clone();
    second.state_id = Uuid::from_u128(12);
    second.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(true, b));
    let mut h = History::new(snapshot(first, 1));
    h.record(snapshot(second, 2)).unwrap();
    assert_eq!(h.memory_usage().filter_mask_bytes, 2 * bytes);
    h.compact(1, 4 << 20, || Ok(())).unwrap();
    assert_eq!(h.memory_usage().filter_mask_bytes, 2 * bytes);
    drop(h);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn canceling_cold_capture_keeps_layer_intent_timeline_and_shared_charge() {
    let budget = SharedBudget::new(8192);
    let field = coverage(&budget);
    let bytes = field.storage_usage().unwrap().1;
    let mut d = masked(base(), field.clone(), 11);
    let id = d.roots[0];
    d.layers.get_mut(&id).unwrap().mask = Some(LayerMask::new(false, field));
    let digest = d.layers[&id].mask.as_ref().unwrap().digest();
    let mut h = History::new(snapshot(d, 1));
    h.record(snapshot(base(), 2)).unwrap();
    let before = h.entries().collect::<Vec<_>>();
    let e = h
        .compact(1, 4 << 20, || {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "layer-mask cold cancel",
            ))
        })
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::OperationCancelled);
    assert_eq!(h.entries().collect::<Vec<_>>(), before);
    let restored = h.undo().unwrap().unwrap();
    assert_eq!(
        restored.document.layers[&id]
            .mask
            .as_ref()
            .unwrap()
            .digest(),
        digest
    );
    assert_eq!(h.memory_usage().filter_mask_bytes, bytes);
    drop((restored, h));
    assert_eq!(budget.usage().reserved, 0);
}
