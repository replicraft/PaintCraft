use paintcraft_core::types::{
    Error, ErrorCode,
    storage::{SpillBlob, SpillStore, SpillUsage},
};
use paintcraft_document::{POLICY, import_image};
use paintcraft_editor::session::Session;
use paintcraft_platform::spill::NativeSpillStore;
use paintcraft_raster::RasterLayer;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
const TILES: usize = 130;
const WIDTH: usize = TILES * 256;
fn pixels() -> Vec<u8> {
    let mut seed = 42u32;
    (0..WIDTH * 256)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed.to_le_bytes()
        })
        .collect()
}
fn scene() -> (Session, Vec<u8>) {
    let pixels = pixels();
    (
        Session::new(import_image(WIDTH as u32, 256, &pixels, "large-recorded-history").unwrap()),
        pixels,
    )
}
fn batch(s: &Session, op: String) -> paintcraft_commands::Batch {
    let t = s.ticket();
    let json = format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{op}]}}"#,
        t.state_id, t.publication_epoch
    );
    paintcraft_commands::Batch::from_json(json.as_bytes()).unwrap()
}
fn apply(s: &mut Session, op: String) -> paintcraft_core::types::Result<()> {
    assert!(s.apply(&batch(s, op))?.committed);
    Ok(())
}
fn dot(s: &mut Session, n: usize) {
    let id = s.document().roots[0];
    apply(s,format!(r##"{{"op":"raster.pencil_path","layer_ref":{{"id":"{id}"}},"points":[{{"x":{},"y":5}}],"color":"#17589CCC","mode":"replace"}}"##,3+n)).unwrap();
}
fn replace(s: &mut Session) -> paintcraft_core::types::Result<()> {
    let id = s.document().roots[0];
    apply(
        s,
        format!(
            r##"{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":{WIDTH},"height":256}},"color":"#FFFFFFFF","mode":"replace"}}"##
        ),
    )
}
fn verify(s: &Session, initial: &[u8], count: usize, white: bool) {
    let doc = s.document();
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.resident_tiles(), TILES);
    for (coord, tile) in layer.raster.tile_leases() {
        for y in 0..256usize {
            let start = (y * WIDTH + coord.tx as usize * 256) * 4;
            let mut expected = initial[start..start + 1024].to_vec();
            if white {
                expected.fill(255);
            } else if coord.tx == 0 && y == 5 {
                for n in 0..count {
                    expected[(3 + n) * 4..(4 + n) * 4].copy_from_slice(&[23, 88, 156, 204]);
                }
            }
            assert_eq!(&tile.bytes()[y * 1024..(y + 1) * 1024], expected);
        }
    }
}
#[derive(Debug)]
struct Counting {
    native: NativeSpillStore,
    writes: AtomicUsize,
    barriers: AtomicUsize,
    fail_after: usize,
}
impl SpillStore for Counting {
    fn write(&self, b: &[u8]) -> paintcraft_core::types::Result<Arc<dyn SpillBlob>> {
        if self.writes.load(Ordering::SeqCst) >= self.fail_after {
            return Err(Error::new(
                ErrorCode::StorageFailed,
                "injected later-batch failure",
            ));
        }
        let blob = self.native.write(b)?;
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(blob)
    }
    fn flush(&self) -> paintcraft_core::types::Result<()> {
        self.native.flush()?;
        self.barriers.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn collect_garbage(&self) -> paintcraft_core::types::Result<SpillUsage> {
        self.native.collect_garbage()
    }
    fn usage(&self) -> SpillUsage {
        self.native.usage()
    }
}
fn store(fail_after: usize) -> (PathBuf, Arc<Counting>) {
    let parent =
        std::env::temp_dir().join(format!("paintcraft-large-history-{}", uuid::Uuid::new_v4()));
    let native = NativeSpillStore::new(&parent, 128 << 20, 4 << 20).unwrap();
    (
        parent,
        Arc::new(Counting {
            native,
            writes: AtomicUsize::new(0),
            barriers: AtomicUsize::new(0),
            fail_after,
        }),
    )
}
#[test]
fn large_shared_base_keeps_all_states_without_spilling_tiles_still_owned_by_hot_documents() {
    let (parent, store) = store(usize::MAX);
    let (mut s, initial) = scene();
    s.set_history_spill_store(store.clone());
    s.set_history_budget((TILES + 4) as u64 * RasterLayer::TILE_BYTES as u64);
    let mut states = vec![s.ticket().state_id];
    for n in 0..32 {
        dot(&mut s, n);
        states.push(s.ticket().state_id);
    }
    assert_eq!(s.undo_depth(), 32);
    assert_eq!(s.trimmed_total(), 0);
    assert!(s.history_memory().logical_payloads > 64);
    assert!(store.writes.load(Ordering::SeqCst) > 0);
    assert!(
        store.writes.load(Ordering::SeqCst) <= 32,
        "unchanged hot base tiles must not be spilled"
    );
    assert!(
        s.history_memory().retained_weight() <= (TILES + 4) as u64 * RasterLayer::TILE_BYTES as u64
    );
    for count in (0..32).rev() {
        s.undo().unwrap().unwrap();
        assert_eq!(s.ticket().state_id, states[count]);
        verify(&s, &initial, count, false);
    }
    for (count, state) in states.iter().enumerate().skip(1) {
        s.redo().unwrap().unwrap();
        assert_eq!(s.ticket().state_id, *state);
        verify(&s, &initial, count, false);
    }
    drop(s);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    std::fs::remove_dir_all(parent).unwrap();
}
#[test]
fn full_surface_change_uses_multiple_transfer_batches_and_can_cool_the_previous_state() {
    let (parent, store) = store(usize::MAX);
    let (mut s, initial) = scene();
    s.set_history_spill_store(store.clone());
    let initial_id = s.ticket().state_id;
    s.set_history_budget((TILES + 1) as u64 * RasterLayer::TILE_BYTES as u64);
    replace(&mut s).unwrap();
    let painted_id = s.ticket().state_id;
    assert_eq!(s.undo_depth(), 1);
    assert_eq!(s.trimmed_total(), 0);
    assert_eq!(store.writes.load(Ordering::SeqCst), TILES);
    assert!(store.barriers.load(Ordering::SeqCst) >= 3);
    assert_eq!(s.history_memory().disk_payloads, TILES);
    assert_eq!(
        s.history_memory().decoded_bytes,
        TILES as u64 * RasterLayer::TILE_BYTES as u64
    );
    verify(&s, &initial, 0, true);
    s.undo().unwrap().unwrap();
    assert_eq!(s.ticket().state_id, initial_id);
    verify(&s, &initial, 0, false);
    s.redo().unwrap().unwrap();
    assert_eq!(s.ticket().state_id, painted_id);
    verify(&s, &initial, 0, true);
    drop(s);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    drop(store);
    std::fs::remove_dir_all(parent).unwrap();
}
#[test]
fn later_batch_failure_keeps_old_document_redo_and_disk_allowance_intact() {
    let (parent, store) = store(64);
    let (mut s, initial) = scene();
    dot(&mut s, 0);
    s.undo().unwrap().unwrap();
    s.set_history_spill_store(store.clone());
    s.set_history_budget((TILES + 1) as u64 * RasterLayer::TILE_BYTES as u64);
    let ticket = s.ticket();
    let view = s.history_view();
    let snapshot = s.snapshot();
    assert_eq!(replace(&mut s).unwrap_err().code, ErrorCode::StorageFailed);
    assert_eq!(store.writes.load(Ordering::SeqCst), 64);
    assert!(store.barriers.load(Ordering::SeqCst) > 0);
    assert_eq!(s.ticket(), ticket);
    assert_eq!(s.history_view(), view);
    assert_eq!(s.redo_depth(), 1);
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    assert!(Arc::ptr_eq(&s.snapshot(), &snapshot));
    verify(&s, &initial, 0, false);
    s.redo().unwrap().unwrap();
    verify(&s, &initial, 1, false);
    drop((snapshot, s, store));
    std::fs::remove_dir_all(parent).unwrap();
}

#[test]
fn deferred_uniform_records_become_inline_and_preserve_hidden_rgb_without_disk() {
    let initial: Vec<u8> = (0..WIDTH * 256).flat_map(|_| [37, 19, 83, 0]).collect();
    let mut s =
        Session::new(import_image(WIDTH as u32, 256, &initial, "large-uniform-history").unwrap());
    s.set_history_budget((TILES + 1) as u64 * RasterLayer::TILE_BYTES as u64);
    replace(&mut s).unwrap();
    assert_eq!(
        s.history_memory().decoded_bytes,
        TILES as u64 * RasterLayer::TILE_BYTES as u64
    );
    assert_eq!(s.history_memory().encoded_bytes, 0);
    assert_eq!(s.history_memory().disk_bytes, 0);
    s.undo().unwrap().unwrap();
    verify(&s, &initial, 0, false);
    s.redo().unwrap().unwrap();
    verify(&s, &initial, 0, true);
}
#[test]
fn deferred_structured_tiles_can_finish_warm_compression_without_a_disk_store() {
    let initial: Vec<u8> = (0..WIDTH * 256)
        .flat_map(|i| {
            [
                (i % 256 / 8) as u8,
                (i / WIDTH / 8) as u8,
                37,
                (i % 4 * 85) as u8,
            ]
        })
        .collect();
    let mut s = Session::new(
        import_image(WIDTH as u32, 256, &initial, "large-compressed-history").unwrap(),
    );
    s.set_history_compression(paintcraft_history::Compression::Lz4);
    let limit = (TILES + 12) as u64 * RasterLayer::TILE_BYTES as u64;
    s.set_history_budget(limit);
    replace(&mut s).unwrap();
    assert!(s.history_memory().compressed_payloads > 64);
    assert!(s.history_memory().retained_weight() <= limit);
    assert_eq!(s.history_memory().disk_bytes, 0);
    s.undo().unwrap().unwrap();
    verify(&s, &initial, 0, false);
    s.redo().unwrap().unwrap();
    verify(&s, &initial, 0, true);
}

#[test]
fn cancelled_worker_after_first_spill_barrier_releases_private_extents_and_keeps_redo() {
    let (parent, store) = store(usize::MAX);
    let (mut s, initial) = scene();
    dot(&mut s, 0);
    let redo = s.ticket().state_id;
    s.undo().unwrap().unwrap();
    s.set_history_spill_store(store.clone());
    s.set_history_budget((TILES + 1) as u64 * RasterLayer::TILE_BYTES as u64);
    let view = s.history_view();
    let doc = s.snapshot();
    let id = s.document().roots[0];
    let batch = batch(
        &s,
        format!(
            r##"{{"op":"raster.fill_rect","layer_ref":{{"id":"{id}"}},"rect":{{"x":0,"y":0,"width":{WIDTH},"height":256}},"color":"#FFFFFFFF","mode":"replace"}}"##
        ),
    );
    let work = s.preparation().unwrap();
    let observed = store.clone();
    let error = std::thread::spawn(move || {
        work.apply(&batch, || {
            if observed.writes.load(Ordering::SeqCst) >= 64
                && observed.barriers.load(Ordering::SeqCst) > 0
            {
                Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "cancel after first flush",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
    })
    .join()
    .unwrap();
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(store.writes.load(Ordering::SeqCst), 64);
    assert!(store.barriers.load(Ordering::SeqCst) > 0);
    assert_eq!(s.history_view(), view);
    assert!(Arc::ptr_eq(&doc, &s.snapshot()));
    assert_eq!(store.collect_garbage().unwrap().reserved_bytes, 0);
    verify(&s, &initial, 0, false);
    assert_eq!(s.redo().unwrap().unwrap().state_id, redo);
    verify(&s, &initial, 1, false);
    drop((s, doc, store));
    std::fs::remove_dir_all(parent).unwrap();
}
