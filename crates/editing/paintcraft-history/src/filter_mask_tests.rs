//! Persistent coverage stays charged and uniquely owned through raster spill.
use super::*;
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba,
    filters::Descriptor,
    resources::SharedBudget,
    storage::{SpillBlob, SpillStore, SpillUsage},
};
use paintcraft_document::{FilterInstance, FilterMask, FilterStack};
use paintcraft_raster::coverage_mask::{A8Mask, Limits};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
fn snapshot(document: Document, id: u128) -> Snapshot {
    Snapshot {
        document: Arc::new(document),
        selection: Selection::Inactive,
        history_id: Some(Uuid::from_u128(id)),
    }
}
fn base() -> Document {
    paintcraft_document::import_image(
        2,
        1,
        &[13, 71, 201, 0, 250, 1, 20, 255],
        "filter-mask-history",
    )
    .unwrap()
}
fn coverage(budget: &SharedBudget) -> A8Mask {
    A8Mask::uniform(
        Rect::new(0, 0, 1, 1).unwrap(),
        127,
        budget,
        Limits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn masked(mut doc: Document, mask: A8Mask, id: u128) -> Document {
    let layer = doc.roots[0];
    doc.layers.get_mut(&layer).unwrap().filters = FilterStack::new(vec![
        FilterInstance::new(Uuid::from_u128(1), true, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(false, mask.clone()))),
        FilterInstance::new(Uuid::from_u128(2), false, Descriptor::InvertRgbV1)
            .with_mask(Some(FilterMask::new(true, mask))),
    ])
    .unwrap();
    doc.state_id = Uuid::from_u128(id);
    doc
}
#[derive(Debug)]
struct Folder(PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[derive(Debug)]
struct DiskBlob {
    path: PathBuf,
    len: usize,
    _owner: Arc<Folder>,
}
impl SpillBlob for DiskBlob {
    fn byte_len(&self) -> usize {
        self.len
    }
    fn read_into(&self, out: &mut [u8]) -> Result<()> {
        if out.len() != self.len {
            return Err(Error::new(ErrorCode::StorageFailed, "spill length"));
        }
        let mut f = fs::File::open(&self.path)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "spill open"))?;
        f.read_exact(out)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "spill read"))
    }
}
#[derive(Debug)]
struct Disk {
    folder: Arc<Folder>,
    bytes: AtomicU64,
}
impl Disk {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("paintcraft-mask-history-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self {
            folder: Arc::new(Folder(path)),
            bytes: AtomicU64::new(0),
        }
    }
}
impl SpillStore for Disk {
    fn write(&self, data: &[u8]) -> Result<Arc<dyn SpillBlob>> {
        let path = self.folder.0.join(Uuid::new_v4().to_string());
        let mut f = fs::File::create(&path)
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "spill create"))?;
        f.write_all(data)
            .and_then(|_| f.sync_all())
            .map_err(|_| Error::new(ErrorCode::StorageFailed, "spill write"))?;
        self.bytes.fetch_add(data.len() as u64, Ordering::Relaxed);
        Ok(Arc::new(DiskBlob {
            path,
            len: data.len(),
            _owner: self.folder.clone(),
        }))
    }
    fn flush(&self) -> Result<()> {
        Ok(())
    }
    fn collect_garbage(&self) -> Result<SpillUsage> {
        Ok(self.usage())
    }
    fn usage(&self) -> SpillUsage {
        SpillUsage {
            reserved_bytes: self.bytes.load(Ordering::Relaxed),
            segments: fs::read_dir(&self.folder.0).unwrap().count(),
            cleanup_failures: 0,
        }
    }
}
#[test]
fn unique_mask_reservation_survives_cold_actual_filesystem_raster_spill_and_undo_redo() {
    let budget = SharedBudget::new(8192);
    let field = coverage(&budget);
    let (identity, bytes) = field.storage_usage().unwrap();
    let first = masked(base(), field.clone(), 11);
    let layer = first.roots[0];
    let weak = Arc::new(first);
    let old = Arc::downgrade(&weak);
    let mut history = History::new(snapshot(base(), 1));
    history
        .record(Snapshot {
            document: weak.clone(),
            selection: Selection::Inactive,
            history_id: Some(Uuid::from_u128(2)),
        })
        .unwrap();
    let mut second = (*weak).clone();
    second.state_id = Uuid::from_u128(12);
    history.record(snapshot(second, 3)).unwrap();
    history
        .record(snapshot(
            paintcraft_document::new_document(2, 1, None, 9, paintcraft_document::POLICY).unwrap(),
            4,
        ))
        .unwrap();
    assert_eq!(history.memory_usage().filter_mask_bytes, bytes);
    history.compact(1, 4 << 20, || Ok(())).unwrap();
    drop(weak);
    drop(field);
    assert!(old.upgrade().is_none());
    assert_eq!(history.memory_usage().filter_mask_bytes, bytes);
    let store = Arc::new(Disk::new());
    let folder = store.folder.0.clone();
    history.set_spill_store(store.clone());
    assert!(history.spill(4 << 20, 16, || Ok(())).unwrap().payloads > 0);
    assert!(store.usage().reserved_bytes > 0);
    assert_eq!(
        budget.usage().reserved,
        bytes,
        "mask remains resident while RGBA is spilled"
    );
    let restored = history.seek(Some(Uuid::from_u128(2))).unwrap().unwrap().0;
    let filters = &restored.document.layers[&layer].filters;
    assert!(!filters.as_slice()[0].mask().unwrap().enabled());
    assert!(!filters.as_slice()[1].enabled());
    assert!(
        filters
            .mask_storage_usage()
            .all(|usage| usage == (identity, bytes))
    );
    assert_eq!(
        filters.as_slice()[0]
            .mask()
            .unwrap()
            .coverage()
            .coverage(0, 0),
        127
    );
    assert_eq!(
        restored.document.layers[&layer].raster.get(0, 0),
        Rgba([13, 71, 201, 0])
    );
    let earlier = history.undo().unwrap().unwrap();
    assert!(
        earlier.document.layers[&earlier.document.roots[0]]
            .filters
            .is_empty()
    );
    let next = history.redo().unwrap().unwrap();
    assert_eq!(
        next.document.layers[&layer].filters,
        restored.document.layers[&layer].filters
    );
    drop((restored, earlier, next, history, store));
    assert_eq!(budget.usage().reserved, 0);
    assert!(!folder.exists());
}
#[test]
fn independently_owned_equal_fields_count_twice_but_cloned_instances_do_not() {
    let budget = SharedBudget::new(8192);
    let a = coverage(&budget);
    let b = coverage(&budget);
    assert_eq!(a, b);
    assert_ne!(a.storage_usage().unwrap().0, b.storage_usage().unwrap().0);
    let bytes = a.storage_usage().unwrap().1;
    let mut history = History::new(snapshot(masked(base(), a, 1), 1));
    history.record(snapshot(masked(base(), b, 2), 2)).unwrap();
    assert_eq!(history.memory_usage().filter_mask_bytes, 2 * bytes);
    let usage = history.memory_usage();
    assert!(usage.known_weight() >= usage.filter_mask_bytes);
    drop(history);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn cancellation_of_cold_capture_keeps_mask_timeline_and_owned_bytes() {
    let budget = SharedBudget::new(8192);
    let mask = coverage(&budget);
    let bytes = mask.storage_usage().unwrap().1;
    let mut history = History::new(snapshot(masked(base(), mask, 1), 1));
    history.record(snapshot(base(), 2)).unwrap();
    let before = history.entries().collect::<Vec<_>>();
    assert_eq!(
        history
            .compact(1, 4 << 20, || Err(Error::new(
                ErrorCode::OperationCancelled,
                "mask cold cancel"
            )))
            .unwrap_err()
            .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(before, history.entries().collect::<Vec<_>>());
    assert_eq!(history.memory_usage().filter_mask_bytes, bytes);
    assert_eq!(budget.usage().reserved, bytes);
    drop(history);
    assert_eq!(budget.usage().reserved, 0);
}
