//! Actual cold-history/spill transfers preserve owned processing intentions.
use super::*;
use paintcraft_core::types::{
    ErrorCode, Rgba,
    filters::{BrightnessContrastV1, Descriptor},
    storage::{SpillBlob, SpillStore, SpillUsage},
};
use paintcraft_document::{FilterInstance, FilterStack};
use std::sync::atomic::{AtomicU64, Ordering};
fn stack(reverse: bool) -> FilterStack {
    let mut entries = vec![
        FilterInstance::new(Uuid::from_u128(1), true, Descriptor::InvertRgbV1),
        FilterInstance::new(
            Uuid::from_u128(2),
            false,
            Descriptor::BrightnessContrastV1(BrightnessContrastV1::new(-19, 23).unwrap()),
        ),
    ];
    if reverse {
        entries.reverse();
    }
    FilterStack::new(entries).unwrap()
}
fn document() -> Document {
    paintcraft_document::import_image(2, 1, &[13, 71, 201, 0, 250, 1, 20, 255], "filter-history")
        .unwrap()
}
fn snapshot(document: Arc<Document>, id: u128) -> Snapshot {
    Snapshot {
        document,
        selection: Selection::Inactive,
        history_id: Some(Uuid::from_u128(id)),
    }
}
#[derive(Debug)]
struct Bytes(Vec<u8>);
impl SpillBlob for Bytes {
    fn byte_len(&self) -> usize {
        self.0.len()
    }
    fn read_into(&self, target: &mut [u8]) -> Result<()> {
        if target.len() != self.0.len() {
            return Err(archive::limit("test spill read extent"));
        }
        target.copy_from_slice(&self.0);
        Ok(())
    }
}
#[derive(Debug, Default)]
struct Store(AtomicU64);
impl SpillStore for Store {
    fn write(&self, bytes: &[u8]) -> Result<Arc<dyn SpillBlob>> {
        self.0.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        Ok(Arc::new(Bytes(bytes.to_vec())))
    }
    fn flush(&self) -> Result<()> {
        Ok(())
    }
    fn collect_garbage(&self) -> Result<SpillUsage> {
        Ok(self.usage())
    }
    fn usage(&self) -> SpillUsage {
        SpillUsage {
            reserved_bytes: self.0.load(Ordering::Relaxed),
            segments: 1,
            cleanup_failures: 0,
        }
    }
}
#[test]
fn cold_spill_undo_redo_reconstruct_exact_order_enablement_and_raw_pixels() {
    let plain = Arc::new(document());
    let mut filtered = (*plain).clone();
    let layer = filtered.roots[0];
    filtered.layers.get_mut(&layer).unwrap().filters = stack(false);
    filtered.state_id = Uuid::from_u128(102);
    let filtered = Arc::new(filtered);
    let mut reordered = (*filtered).clone();
    reordered.layers.get_mut(&layer).unwrap().filters = stack(true);
    reordered.state_id = Uuid::from_u128(103);
    let reordered = Arc::new(reordered);
    let versions = |d: &Document| {
        d.layers[&layer]
            .raster
            .tile_leases()
            .map(|(c, t)| (c, t.version()))
            .collect::<Vec<_>>()
    };
    assert!(!versions(&plain).is_empty());
    assert_eq!(versions(&plain), versions(&filtered));
    assert_eq!(versions(&plain), versions(&reordered));
    let weak = Arc::downgrade(&filtered);
    let mut history = History::new(snapshot(plain.clone(), 1));
    history.record(snapshot(filtered.clone(), 2)).unwrap();
    history.record(snapshot(reordered.clone(), 3)).unwrap();
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
        history
            .compact(1, 4 << 20, || Ok(()))
            .unwrap()
            .packed_entries,
        3
    );
    drop((plain, filtered, reordered));
    assert!(
        weak.upgrade().is_none(),
        "must reconstruct cold document, not reuse live snapshot"
    );
    let store = Arc::new(Store::default());
    history.set_spill_store(store.clone());
    let spilled = history.spill(4 << 20, 16, || Ok(())).unwrap();
    assert!(spilled.payloads > 0);
    assert!(store.usage().reserved_bytes > 0);
    let restored = history.seek(Some(Uuid::from_u128(2))).unwrap().unwrap().0;
    assert_eq!(restored.document.layers[&layer].filters, stack(false));
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
    let next = history.redo().unwrap().unwrap();
    assert_eq!(next.document.layers[&layer].filters, stack(false));
    let reordered = history.redo().unwrap().unwrap();
    assert_eq!(reordered.document.layers[&layer].filters, stack(true));
    assert_eq!(versions(&restored.document), versions(&next.document));
    assert_eq!(versions(&restored.document), versions(&reordered.document));
}
#[test]
fn owned_stack_allocations_are_counted_per_snapshot_and_in_cold_metadata() {
    let plain = document();
    let id = plain.roots[0];
    let mut filtered = plain.clone();
    filtered.layers.get_mut(&id).unwrap().filters = stack(false);
    let bytes = filtered.layers[&id].filters.storage_bytes();
    assert!(bytes > 0);
    assert_eq!(
        archive::document_metadata(&filtered) - archive::document_metadata(&plain),
        bytes
    );
    let timeline = |document: &Document| {
        let mut history = History::new(snapshot(Arc::new(document.clone()), 1));
        history
            .record(snapshot(Arc::new(document.clone()), 2))
            .unwrap();
        history
    };
    let plain_history = timeline(&plain);
    let history = timeline(&filtered);
    assert_eq!(
        history.memory_usage().metadata_weight - plain_history.memory_usage().metadata_weight,
        (2 * bytes) as u64
    );
    let packed = |document: &Document| {
        let mut sources = usize::MAX;
        let mut deferred = 0;
        archive::Packed::build(
            &snapshot(Arc::new(document.clone()), 1),
            &mut archive::Memo::new(),
            &mut (4 << 20),
            &|| Ok(()),
            &mut compression::Encoder::new(Compression::None),
            &mut sources,
            &mut deferred,
        )
        .unwrap()
    };
    assert_eq!(
        packed(&filtered).metadata_weight() - packed(&plain).metadata_weight(),
        bytes
    );
}
#[test]
fn failed_cold_capture_keeps_stack_and_timeline_unchanged() {
    let mut document = document();
    let id = document.roots[0];
    document.layers.get_mut(&id).unwrap().filters = stack(false);
    let mut history = History::new(snapshot(Arc::new(document), 1));
    history
        .record(snapshot(
            Arc::new(
                paintcraft_document::new_document(2, 1, None, 0, paintcraft_document::POLICY)
                    .unwrap(),
            ),
            2,
        ))
        .unwrap();
    let before: Vec<_> = history.entries().collect();
    assert_eq!(
        history.compact(1, 0, || Ok(())).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(history.entries().collect::<Vec<_>>(), before);
    let original = history.undo().unwrap().unwrap();
    assert_eq!(original.document.layers[&id].filters, stack(false));
}

#[path = "filter_tests/catalog_tests.rs"]
mod catalog_tests;

#[path = "filter_tests/exposure_tests.rs"]
mod exposure_tests;

#[path = "filter_tests/curves_tests.rs"]
mod curves_tests;
