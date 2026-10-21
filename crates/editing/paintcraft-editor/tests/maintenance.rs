use paintcraft_core::types::{Error, ErrorCode, Rgba};
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::Session;
use paintcraft_raster::RasterLayer;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
fn session() -> Session {
    Session::new(new_document(8, 8, Some(Rgba([255; 4])), 0, POLICY).unwrap())
}
fn paint(session: &mut Session, x: u32) {
    let ticket = session.ticket();
    let id = session.document().roots[0];
    session.apply_json(format!(r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":{x},"y":0,"width":1,"height":1}},"color":"#010203FF","mode":"replace"}}]}}"##,ticket.state_id,ticket.publication_epoch).as_bytes()).unwrap();
}
#[test]
fn publication_reclaims_owner_history_without_editing_revision_cursor_or_save_acknowledgement() {
    let mut s = session();
    paint(&mut s, 0);
    let old = s.document().layers[&s.document().roots[0]]
        .raster
        .tile_leases()
        .next()
        .unwrap()
        .1
        .version();
    for x in 1..3 {
        paint(&mut s, x);
    }
    let ticket = s.ticket();
    let view = s.history_view();
    let doc = s.snapshot();
    let bytes = RasterLayer::TILE_BYTES;
    let source = s.document().layers[&s.document().roots[0]]
        .raster
        .tile_leases()
        .next()
        .unwrap()
        .1
        .version();
    let prepared = std::thread::spawn({
        let work = s.preparation().unwrap();
        move || work.maintain_history(|| Ok(())).unwrap()
    })
    .join()
    .unwrap();
    assert_eq!(s.history_view(), view);
    assert_eq!(s.ticket(), ticket);
    assert!(source.try_lease().is_some());
    assert_eq!(old.resident_bytes(), bytes);
    s.mark_saved();
    let report = s.publish_history_maintenance(prepared, || Ok(())).unwrap();
    assert!(report.made_progress());
    assert_eq!(old.resident_bytes(), 0);
    assert!(!s.is_dirty());
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), view);
    assert!(Arc::ptr_eq(&s.snapshot(), &doc));
    s.undo().unwrap().unwrap();
    let raster = &s.document().layers[&s.document().roots[0]].raster;
    assert_eq!(raster.get(1, 0), Rgba([1, 2, 3, 255]));
    assert_eq!(raster.get(2, 0), Rgba([255; 4]));
    s.redo().unwrap().unwrap();
    assert!(!s.is_dirty());
    assert_eq!(
        s.document().layers[&s.document().roots[0]].raster.get(2, 0),
        Rgba([1, 2, 3, 255])
    );
    drop(s);
    assert_eq!(source.resident_bytes(), bytes);
    drop(doc);
    assert_eq!(source.resident_bytes(), 0);
}
#[test]
fn stale_foreign_policy_changed_and_superseded_maintenance_cannot_replace_history() {
    let mut s = session();
    for x in 0..3 {
        paint(&mut s, x);
    }
    let first = s
        .preparation()
        .unwrap()
        .maintain_history(|| Ok(()))
        .unwrap();
    let old = s
        .preparation()
        .unwrap()
        .maintain_history(|| Ok(()))
        .unwrap();
    let ticket = s.ticket();
    s.publish_history_maintenance(first, || Ok(())).unwrap();
    assert_eq!(s.ticket(), ticket);
    assert_eq!(
        s.publish_history_maintenance(old, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let old = s
        .preparation()
        .unwrap()
        .maintain_history(|| Ok(()))
        .unwrap();
    paint(&mut s, 4);
    let view = s.history_view();
    assert_eq!(
        s.publish_history_maintenance(old, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    assert_eq!(s.history_view(), view);
    let old = s
        .preparation()
        .unwrap()
        .maintain_history(|| Ok(()))
        .unwrap();
    s.set_history_budget(1 << 20);
    assert_eq!(
        s.publish_history_maintenance(old, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
    let source = session();
    let mut foreign = session();
    assert_eq!(source.ticket(), foreign.ticket());
    let empty = source
        .preparation()
        .unwrap()
        .maintain_history(|| Ok(()))
        .unwrap();
    assert_eq!(
        foreign
            .publish_history_maintenance(empty, || Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ConflictStaleBase
    );
}
#[test]
fn every_preparation_cancellation_boundary_keeps_original_history_and_redo() {
    let mut s = session();
    for x in 0..4 {
        paint(&mut s, x);
    }
    s.undo().unwrap().unwrap();
    let ticket = s.ticket();
    let view = s.history_view();
    let count = AtomicUsize::new(0);
    let result = s
        .preparation()
        .unwrap()
        .maintain_history(|| {
            count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    drop(result);
    for stop in 1..=count.load(Ordering::Relaxed) {
        let count = AtomicUsize::new(0);
        let error = s
            .preparation()
            .unwrap()
            .maintain_history(|| {
                if count.fetch_add(1, Ordering::Relaxed) + 1 == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "injected maintenance cancellation",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(s.ticket(), ticket);
        assert_eq!(s.history_view(), view);
    }
    let ready = s
        .preparation()
        .unwrap()
        .maintain_history(|| Ok(()))
        .unwrap();
    assert_eq!(
        s.publish_history_maintenance(ready, || Err(Error::new(
            ErrorCode::OperationCancelled,
            "cancelled before publication"
        )))
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(s.history_view(), view);
    s.redo().unwrap().unwrap();
    assert_eq!(
        s.document().layers[&s.document().roots[0]].raster.get(3, 0),
        Rgba([1, 2, 3, 255])
    );
}

#[test]
fn failed_required_spill_barrier_keeps_published_history_and_pixels_available() {
    use paintcraft_core::types::storage::{SpillBlob, SpillStore, SpillUsage};
    #[derive(Debug)]
    struct Bytes(Vec<u8>);
    impl SpillBlob for Bytes {
        fn byte_len(&self) -> usize {
            self.0.len()
        }
        fn read_into(&self, out: &mut [u8]) -> paintcraft_core::types::Result<()> {
            out.copy_from_slice(&self.0);
            Ok(())
        }
    }
    #[derive(Debug)]
    struct FailedBarrier;
    impl SpillStore for FailedBarrier {
        fn write(&self, bytes: &[u8]) -> paintcraft_core::types::Result<Arc<dyn SpillBlob>> {
            Ok(Arc::new(Bytes(bytes.to_vec())))
        }
        fn flush(&self) -> paintcraft_core::types::Result<()> {
            Err(Error::new(
                ErrorCode::StorageFailed,
                "injected maintenance spill barrier",
            ))
        }
        fn collect_garbage(&self) -> paintcraft_core::types::Result<SpillUsage> {
            Ok(SpillUsage::default())
        }
        fn usage(&self) -> SpillUsage {
            SpillUsage::default()
        }
    }
    let mut s = session();
    for x in 0..3 {
        paint(&mut s, x);
    }
    s.set_history_spill_store(Arc::new(FailedBarrier));
    let view = s.history_view();
    let doc = s.snapshot();
    let ticket = s.ticket();
    assert_eq!(
        s.preparation()
            .unwrap()
            .maintain_history(|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::StorageFailed
    );
    assert_eq!(s.history_view(), view);
    assert_eq!(s.ticket(), ticket);
    assert!(Arc::ptr_eq(&s.snapshot(), &doc));
    s.undo().unwrap().unwrap();
    s.redo().unwrap().unwrap();
    assert_eq!(
        s.document().layers[&s.document().roots[0]].raster.get(2, 0),
        Rgba([1, 2, 3, 255])
    );
}
