use paintcraft_core::types::storage::{SpillStore, SpillUsage};
use paintcraft_document::{POLICY, import_image};
use paintcraft_editor::session::Session;
use paintcraft_history::MemoryUsage;
use paintcraft_platform::spill::NativeSpillStore;
use paintcraft_raster::{RasterLayer, TileVersion};
use std::{sync::Arc, time::Instant};
pub const CASES: &[&str] = &["large65", "large130", "large260"];
pub const VARIANTS: &[&str] = &["automatic", "explicit"];
pub fn tile_count(case: &str) -> usize {
    match case {
        "large65" => 65,
        "large130" => 130,
        "large260" => 260,
        _ => panic!("unknown case"),
    }
}
struct Reference {
    crc: Vec<u32>,
    first_tile: Vec<u8>,
}
fn fixture_reference(case: &str) -> (Session, Reference) {
    let tiles = tile_count(case);
    let width = tiles * 256;
    let mut seed = 42u32;
    let pixels: Vec<u8> = (0..width * 256)
        .flat_map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed.to_le_bytes()
        })
        .collect();
    let mut hashes: Vec<_> = (0..tiles).map(|_| crc32fast::Hasher::new()).collect();
    let mut first_tile = Vec::with_capacity(RasterLayer::TILE_BYTES);
    for y in 0..256 {
        for (x, hash) in hashes.iter_mut().enumerate() {
            let start = (y * width + x * 256) * 4;
            hash.update(&pixels[start..start + 1024]);
            if x == 0 {
                first_tile.extend_from_slice(&pixels[start..start + 1024]);
            }
        }
    }
    let crc = hashes.into_iter().map(|h| h.finalize()).collect();
    (
        Session::new(
            import_image(width as u32, 256, &pixels, "large-history-process-fixture").unwrap(),
        ),
        Reference { crc, first_tile },
    )
}
pub fn fixture(case: &str) -> Session {
    fixture_reference(case).0
}
#[derive(Debug)]
pub struct Outcome {
    pub memory: MemoryUsage,
    pub store: SpillUsage,
    pub events: Vec<(&'static str, usize, u128)>,
}
fn event(
    events: &mut Vec<(&'static str, usize, u128)>,
    trace: bool,
    phase: &'static str,
    n: usize,
    start: Instant,
) {
    if trace {
        events.push((phase, n, start.elapsed().as_nanos()));
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fingerprint {
    id: uuid::Uuid,
    changed: u32,
}
fn fingerprint(
    s: &Session,
    base: &[TileVersion],
    reference: &Reference,
    full: bool,
) -> Fingerprint {
    let doc = s.document();
    let layer = &doc.layers[&doc.roots[0]];
    assert_eq!(layer.raster.resident_tiles(), base.len());
    assert_eq!(layer.opacity, 255);
    let mut crc = None;
    for (coord, tile) in layer.raster.tile_leases() {
        assert_eq!(coord.ty, 0);
        if coord.tx == 0 {
            crc = Some(crc32fast::hash(tile.bytes()));
        } else {
            if full {
                assert_eq!(
                    crc32fast::hash(tile.bytes()),
                    reference.crc[coord.tx as usize],
                    "independent unchanged bytes"
                );
            }
            assert_eq!(
                tile.version(),
                base[coord.tx as usize],
                "unchanged base identity"
            );
        }
    }
    Fingerprint {
        id: doc.state_id,
        changed: crc.unwrap(),
    }
}
pub fn run(case: &str, variant: &str, trace: bool) -> Outcome {
    run_checked(case, variant, trace, false)
}
pub fn verify(case: &str, variant: &str) -> Outcome {
    run_checked(case, variant, false, true)
}
fn run_checked(case: &str, variant: &str, trace: bool, full: bool) -> Outcome {
    assert!(VARIANTS.contains(&variant));
    let tiles = tile_count(case);
    let (mut s, mut reference) = fixture_reference(case);
    let base: Vec<_> = {
        let doc = s.document();
        let layer = &doc.layers[&doc.roots[0]];
        let mut versions: Vec<_> = layer
            .raster
            .tile_leases()
            .map(|(c, t)| (c.tx, t.version()))
            .collect();
        versions.sort_by_key(|(x, _)| *x);
        versions.into_iter().map(|(_, v)| v).collect()
    };
    // Full preflight checks use generator-derived row CRCs. Timed replays check
    // the changed bytes independently and all unchanged immutable identities,
    // with full unchanged-byte checks at both ends of recorded navigation.
    let first = fingerprint(&s, &base, &reference, true);
    assert_eq!(first.changed, reference.crc[0]);
    let native = Arc::new(
        NativeSpillStore::new(
            &std::env::temp_dir().join("paintcraft-large-history-bench"),
            128 << 20,
            4 << 20,
        )
        .unwrap(),
    );
    let directory = native.directory().to_owned();
    s.set_history_spill_store(native.clone());
    let budget = (tiles + 4) as u64 * RasterLayer::TILE_BYTES as u64;
    s.set_history_budget(budget);
    let mut expected = vec![first];
    let mut events = Vec::new();
    for n in 0..32 {
        let start = Instant::now();
        let ticket = s.ticket();
        let id = s.document().roots[0];
        let batch = format!(
            r##"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{{"op":"raster.pencil_path","layer_ref":{{"id":"{id}"}},"points":[{{"x":{},"y":5}}],"color":"#17589CCC","mode":"replace"}}]}}"##,
            ticket.state_id,
            ticket.publication_epoch,
            3 + n
        );
        assert!(s.apply_json(batch.as_bytes()).unwrap().committed);
        event(&mut events, trace, "record", n, start);
        if variant == "explicit" {
            let begin = Instant::now();
            s.compact_history(2, 64 << 20, || Ok(())).unwrap();
            s.spill_history(64 << 20, usize::MAX, || Ok(())).unwrap();
            event(&mut events, trace, "explicit-preparation", n, begin);
        }
        let offset = (5 * 256 + 3 + n) * 4;
        reference.first_tile[offset..offset + 4].copy_from_slice(&[23, 88, 156, 204]);
        let got = fingerprint(&s, &base, &reference, full);
        assert_eq!(
            got.changed,
            crc32fast::hash(&reference.first_tile),
            "independent changed bytes"
        );
        expected.push(got);
        assert_eq!(s.trimmed_total(), 0);
        assert_eq!(s.undo_depth(), n + 1);
        assert!(s.history_memory().retained_weight() <= budget);
        event(&mut events, trace, "edit-cycle", n, start);
    }
    fingerprint(&s, &base, &reference, true);
    let memory = s.history_memory();
    let store = native.usage();
    for (n, want) in expected[..32].iter().enumerate().rev() {
        let start = Instant::now();
        s.undo().unwrap().unwrap();
        event(&mut events, trace, "undo", n, start);
        assert_eq!(fingerprint(&s, &base, &reference, full), *want);
    }
    for (n, want) in expected.iter().enumerate().skip(1) {
        let start = Instant::now();
        s.redo().unwrap().unwrap();
        event(&mut events, trace, "redo", n, start);
        assert_eq!(fingerprint(&s, &base, &reference, full), *want);
    }
    fingerprint(&s, &base, &reference, true);
    let start = Instant::now();
    drop(s);
    assert_eq!(native.collect_garbage().unwrap().reserved_bytes, 0);
    drop(native);
    assert!(!directory.exists());
    event(&mut events, trace, "destruction", 0, start);
    Outcome {
        memory,
        store,
        events,
    }
}
