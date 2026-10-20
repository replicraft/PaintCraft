use paintcraft_core::types::{
    Error, ErrorCode,
    storage::{SpillBlob, SpillStore, SpillUsage},
};
use paintcraft_document::{POLICY, import_image};
use paintcraft_editor::session::Session;
use paintcraft_history::Compression;
use paintcraft_platform::spill::NativeSpillStore;
use paintcraft_raster::RasterLayer;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
fn pixels(noise: bool) -> Vec<u8> {
    let mut seed = 42u32;
    (0..65536)
        .flat_map(|i| {
            if noise {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                seed.to_le_bytes()
            } else {
                [
                    (i % 256 / 8) as u8,
                    (i / 256 / 8) as u8,
                    37,
                    (i % 4 * 85) as u8,
                ]
            }
        })
        .collect()
}
fn session(noise: bool) -> Session {
    Session::new(import_image(256, 256, &pixels(noise), "disk-history-fixture").unwrap())
}
fn paint(s: &mut Session, n: usize) -> paintcraft_core::types::Result<()> {
    let t = s.ticket();
    let id = s.document().roots[0];
    let batch = format!(
        r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.pencil_path","layer_ref":{{"id":"{id}"}},"points":[{{"x":{},"y":5}}],"color":"#17589CCC","mode":"replace"}}]}}"##,
        t.state_id,
        t.publication_epoch,
        3 + n
    );
    assert!(s.apply_json(batch.as_bytes())?.committed);
    Ok(())
}
fn store(quota: u64) -> (PathBuf, Arc<NativeSpillStore>) {
    let parent =
        std::env::temp_dir().join(format!("paintcraft-history-spill-{}", uuid::Uuid::new_v4()));
    let store = Arc::new(NativeSpillStore::new(&parent, quota, 1 << 20).unwrap());
    (parent, store)
}
fn verify(s: &Session, noise: bool, count: usize) {
    let mut expected = pixels(noise);
    for n in 0..count {
        let i = (5 * 256 + 3 + n) * 4;
        expected[i..i + 4].copy_from_slice(&[23, 88, 156, 204]);
    }
    let doc = s.document();
    let tile = doc.layers[&doc.roots[0]]
        .raster
        .tile_leases()
        .next()
        .unwrap()
        .1;
    assert_eq!(tile.bytes(), expected);
}
#[test]
fn pressure_spills_raw_history_without_trimming_and_readers_keep_their_pixels() {
    let (parent, store) = store(32 << 20);
    let mut s = session(true);
    s.set_history_spill_store(store.clone());
    s.set_history_budget(4 * RasterLayer::TILE_BYTES as u64);
    let held = s.snapshot();
    let version = held.layers[&held.roots[0]]
        .raster
        .tile_leases()
        .next()
        .unwrap()
        .1
        .version();
    let mut states = vec![s.ticket().state_id];
    for n in 0..40 {
        paint(&mut s, n).unwrap();
        states.push(s.ticket().state_id);
    }
    assert_eq!(s.undo_depth(), 40);
    assert_eq!(s.trimmed_total(), 0);
    assert!(s.history_memory().disk_payloads > 0);
    assert!(s.history_memory().leased_decoded_bytes > 0);
    assert_eq!(version.resident_bytes(), RasterLayer::TILE_BYTES);
    drop(held);
    assert_eq!(version.resident_bytes(), 0);
    for count in (0..40).rev() {
        s.undo().unwrap().unwrap();
        assert_eq!(s.ticket().state_id, states[count]);
        verify(&s, true, count);
    }
    for (count, state) in states.iter().enumerate().skip(1) {
        s.redo().unwrap().unwrap();
        assert_eq!(s.ticket().state_id, *state);
        verify(&s, true, count);
    }
    drop(s);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    std::fs::remove_dir_all(parent).unwrap();
}
#[test]
fn staged_disk_failure_rejects_an_edit_and_keeps_the_redo_branch() {
    let mut s = session(true);
    for n in 0..3 {
        paint(&mut s, n).unwrap();
    }
    s.undo().unwrap().unwrap();
    let ticket = s.ticket();
    let view = s.history_view();
    s.set_history_budget(0);
    assert_eq!(
        paint(&mut s, 20).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), view);
    verify(&s, true, 2);
    s.set_history_budget(1 << 30);
    paint(&mut s, 20).unwrap();
    assert_eq!(s.redo_depth(), 0);
    s.undo().unwrap().unwrap();
    assert_eq!(s.ticket().state_id, ticket.state_id);
    verify(&s, true, 2);
}
#[test]
fn zero_disk_allowance_never_publishes_or_trims_and_a_new_store_allows_retry() {
    let (parent, empty) = store(0);
    let mut s = session(true);
    s.set_history_spill_store(empty.clone());
    s.set_history_budget(3 * RasterLayer::TILE_BYTES as u64);
    paint(&mut s, 0).unwrap();
    let ticket = s.ticket();
    let view = s.history_view();
    assert_eq!(paint(&mut s, 1).unwrap_err().code, ErrorCode::ResourceLimit);
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), view);
    assert_eq!(empty.usage().reserved_bytes, 0);
    verify(&s, true, 1);
    let (other, usable) = store(16 << 20);
    s.set_history_spill_store(usable.clone());
    paint(&mut s, 1).unwrap();
    assert_eq!(s.undo_depth(), 2);
    assert_eq!(s.trimmed_total(), 0);
    drop((s, empty, usable));
    std::fs::remove_dir_all(parent).unwrap();
    std::fs::remove_dir_all(other).unwrap();
}
#[test]
fn every_payload_policy_spills_in_bounded_batches_and_restores_exact_recorded_results() {
    for codec in [
        Compression::None,
        Compression::Lz4,
        Compression::Zstd1,
        Compression::Zstd3,
    ] {
        let (parent, store) = store(16 << 20);
        let mut s = session(false);
        s.set_history_spill_store(store.clone());
        s.set_history_compression(codec);
        for n in 0..10 {
            paint(&mut s, n).unwrap();
        }
        s.compact_history(1, 16 << 20, || Ok(())).unwrap();
        assert_eq!(s.spill_history(16 << 20, 4, || Ok(())).unwrap().payloads, 4);
        assert_eq!(s.history_memory().disk_payloads, 4);
        s.spill_history(16 << 20, 64, || Ok(())).unwrap();
        assert_eq!(s.history_memory().disk_payloads, 10);
        assert_eq!(s.history_memory().encoded_bytes, 0);
        assert_eq!(
            s.history_memory().decoded_bytes,
            RasterLayer::TILE_BYTES as u64
        );
        for count in (0..10).rev() {
            s.undo().unwrap().unwrap();
            verify(&s, false, count);
        }
        for count in 1..=10 {
            s.redo().unwrap().unwrap();
            verify(&s, false, count);
        }
        drop(s);
        assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
        drop(store);
        std::fs::remove_dir_all(parent).unwrap();
    }
}
#[derive(Debug)]
struct Failing {
    native: Arc<NativeSpillStore>,
    remaining: AtomicUsize,
    fail_flush: std::sync::atomic::AtomicBool,
    flushes: AtomicUsize,
}
impl SpillStore for Failing {
    fn write(&self, b: &[u8]) -> paintcraft_core::types::Result<Arc<dyn SpillBlob>> {
        if self
            .remaining
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_err()
        {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                "injected disk-full failure",
            ));
        }
        self.native.write(b)
    }
    fn flush(&self) -> paintcraft_core::types::Result<()> {
        if self.fail_flush.swap(false, Ordering::SeqCst) {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                "injected sync failure",
            ));
        }
        self.native.flush()?;
        self.flushes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn collect_garbage(&self) -> paintcraft_core::types::Result<SpillUsage> {
        self.native.collect_garbage()
    }
    fn usage(&self) -> SpillUsage {
        self.native.usage()
    }
}
#[test]
fn failed_or_cancelled_spill_does_not_replace_any_entry_and_reclaims_provisional_segments() {
    let (parent, native) = store(16 << 20);
    let failing = Arc::new(Failing {
        native: native.clone(),
        remaining: AtomicUsize::new(2),
        fail_flush: std::sync::atomic::AtomicBool::new(false),
        flushes: AtomicUsize::new(0),
    });
    let mut s = session(true);
    for n in 0..10 {
        paint(&mut s, n).unwrap();
    }
    s.compact_history(1, 16 << 20, || Ok(())).unwrap();
    s.set_history_spill_store(failing.clone());
    let ticket = s.ticket();
    let view = s.history_view();
    let memory = s.history_memory();
    assert_eq!(
        s.spill_history(16 << 20, 64, || Ok(())).unwrap_err().code,
        ErrorCode::StorageFailed
    );
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), view);
    assert_eq!(s.history_memory(), memory);
    assert_eq!(native.usage().reserved_bytes, 0);
    failing.remaining.store(100, Ordering::SeqCst);
    assert_eq!(
        s.spill_history(16 << 20, 64, || if native.usage().reserved_bytes > 0 {
            Err(Error::new(ErrorCode::OperationCancelled, "after write"))
        } else {
            Ok(())
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(s.history_view(), view);
    assert_eq!(s.history_memory(), memory);
    assert_eq!(native.usage().reserved_bytes, 0);
    failing.fail_flush.store(true, Ordering::SeqCst);
    assert_eq!(
        s.spill_history(16 << 20, 64, || Ok(())).unwrap_err().code,
        ErrorCode::StorageFailed
    );
    assert_eq!(s.history_view(), view);
    assert_eq!(s.history_memory(), memory);
    assert_eq!(native.usage().reserved_bytes, 0);
    assert_eq!(
        s.spill_history(16 << 20, 64, || {
            if failing.flushes.load(Ordering::SeqCst) != 0 {
                Err(Error::new(ErrorCode::OperationCancelled, "after sync"))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(s.history_view(), view);
    assert_eq!(s.history_memory(), memory);
    assert_eq!(native.usage().reserved_bytes, 0);
    failing.remaining.store(100, Ordering::SeqCst);
    s.spill_history(16 << 20, 64, || Ok(())).unwrap();
    s.undo().unwrap().unwrap();
    verify(&s, true, 9);
    drop((s, failing, native));
    std::fs::remove_dir_all(parent).unwrap();
}
#[test]
fn missing_segment_failure_keeps_published_document_cursor_and_epoch_unchanged() {
    let (parent, store) = store(16 << 20);
    let mut s = session(true);
    s.set_history_spill_store(store.clone());
    for n in 0..3 {
        paint(&mut s, n).unwrap();
    }
    s.compact_history(1, 16 << 20, || Ok(())).unwrap();
    s.spill_history(16 << 20, 64, || Ok(())).unwrap();
    let ticket = s.ticket();
    let view = s.history_view();
    for file in std::fs::read_dir(store.directory()).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_some_and(|x| x == "segment") {
            std::fs::remove_file(path).unwrap();
        }
    }
    assert_eq!(s.undo().unwrap_err().code, ErrorCode::StorageFailed);
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), view);
    verify(&s, true, 3);
    drop((s, store));
    std::fs::remove_dir_all(parent).unwrap();
}
