use paintcraft_core::types::Rgba;
use paintcraft_document::{POLICY, new_document};
use paintcraft_editor::session::{HistoryPacking, Session};
use paintcraft_history::MemoryUsage;
use std::{sync::Arc, time::Instant};

pub const CASES: &[&str] = &[
    "sparse256",
    "sparse1000",
    "held1000",
    "noise128",
    "metadata1000",
];
pub const VARIANTS: &[&str] = &["shared", "pack32", "pack64", "pack128"];
pub fn count(case: &str) -> usize {
    match case {
        "sparse256" => 256,
        "noise128" => 128,
        _ => 1000,
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub phase: &'static str,
    pub index: usize,
    pub ns: u128,
}
#[derive(Debug)]
pub struct Outcome {
    pub before_release: MemoryUsage,
    pub after_release: MemoryUsage,
    pub events: Vec<Event>,
    pub rss_before_release_kib: Option<u64>,
    pub rss_after_release_kib: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fingerprint {
    state: uuid::Uuid,
    pixels: u32,
    opacity: u8,
}
fn fingerprint(s: &Session) -> Fingerprint {
    let doc = s.document();
    let layer = &doc.layers[&doc.roots[0]];
    let mut leases: Vec<_> = layer.raster.tile_leases().collect();
    leases.sort_by_key(|(c, _)| (c.ty, c.tx));
    let mut crc = crc32fast::Hasher::new();
    for (_, lease) in leases {
        crc.update(lease.bytes());
    }
    Fingerprint {
        state: doc.state_id,
        pixels: crc.finalize(),
        opacity: layer.opacity,
    }
}
pub fn fixture(case: &str) -> Session {
    let doc = if case == "noise128" {
        let mut state = 42u32;
        let pixels: Vec<u8> = (0..65536)
            .flat_map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()
            })
            .collect();
        paintcraft_document::import_image(256, 256, &pixels, "recorded-history-noise").unwrap()
    } else {
        new_document(257, 259, Some(Rgba([37, 19, 83, 0])), 42, POLICY).unwrap()
    };
    let mut s = Session::new(doc);
    if case == "metadata1000" {
        apply(&mut s, "sparse256", 0);
    }
    s.set_history_budget(768 << 20);
    s.set_history_packing(HistoryPacking::Shared);
    s
}
fn apply(s: &mut Session, case: &str, n: usize) {
    let t = s.ticket();
    let layer = s.document().roots[0];
    let op = if case == "metadata1000" {
        format!(
            r#"{{"op":"layer.set_properties","layer_ref":{{"id":"{layer}"}},"opacity_u8":{}}}"#,
            if n.is_multiple_of(2) { 73 } else { 255 }
        )
    } else {
        format!(
            r##"{{"op":"raster.pencil_path","layer_ref":{{"id":"{layer}"}},"points":[{{"x":{},"y":{}}}],"color":"#17589CCC","mode":"replace"}}"##,
            3 + n % 120,
            5 + n / 120
        )
    };
    let json = format!(
        r#"{{"schema":"paintcraft.batch/v2","atomic":true,"policy":"{POLICY}","base":{{"state_id":"{}","publication_epoch":"{}"}},"operations":[{op}]}}"#,
        t.state_id, t.publication_epoch
    );
    assert!(s.apply_json(json.as_bytes()).unwrap().committed);
}
fn event(events: &mut Vec<Event>, trace: bool, phase: &'static str, index: usize, start: Instant) {
    if trace {
        events.push(Event {
            phase,
            index,
            ns: start.elapsed().as_nanos(),
        });
    }
}
/// Equal retained states and exact recorded undo/redo output for every strategy.
/// All histories have the same 768 MiB retention allowance. Explicit cold-age
/// checkpoints are the comparison dimension, not a smaller baseline undo chain.
pub fn run(case: &str, variant: &str, trace: bool) -> Outcome {
    assert!(CASES.contains(&case));
    assert!(VARIANTS.contains(&variant));
    let n = count(case);
    let interval = match variant {
        "pack32" => 32,
        "pack128" => 128,
        "pack64" => 64,
        _ => usize::MAX,
    };
    let mut s = fixture(case);
    let baseline_depth = s.undo_depth();
    let mut expected = Vec::with_capacity(n + 1);
    expected.push(fingerprint(&s));
    let mut held: Vec<Arc<paintcraft_document::Document>> = Vec::new();
    let mut events = Vec::with_capacity(if trace { n * 3 + 128 } else { 0 });
    for i in 0..n {
        let start = Instant::now();
        apply(&mut s, case, i);
        event(&mut events, trace, "commit", i, start);
        expected.push(fingerprint(&s));
        if case == "held1000" && i.is_multiple_of(8) {
            held.push(s.snapshot());
        }
        if (i + 1).is_multiple_of(interval) {
            let start = Instant::now();
            s.compact_history(2, 16 << 20, || Ok(())).unwrap();
            event(&mut events, trace, "compact", i, start);
        }
        event(&mut events, trace, "edit-cycle", i, start);
    }
    if variant != "shared" {
        let start = Instant::now();
        s.compact_history(2, 16 << 20, || Ok(())).unwrap();
        event(&mut events, trace, "compact", n, start);
    }
    assert_eq!(s.trimmed_total(), 0);
    assert_eq!(s.undo_depth(), baseline_depth + n);
    let before_release = s.history_memory();
    let rss_before_release_kib = if trace { rss_kib() } else { None };
    let start = Instant::now();
    drop(held);
    event(&mut events, trace, "release-readers", 0, start);
    if variant != "shared" {
        let start = Instant::now();
        s.compact_history(2, 16 << 20, || Ok(())).unwrap();
        event(&mut events, trace, "reclaim", 0, start);
    }
    let rss_after_release_kib = if trace { rss_kib() } else { None };
    let after_release = s.history_memory();
    for (i, want) in expected[..n].iter().enumerate().rev() {
        let start = Instant::now();
        s.undo().unwrap().unwrap();
        event(&mut events, trace, "undo", i, start);
        assert_eq!(fingerprint(&s), *want);
    }
    if baseline_depth != 0 {
        let start = Instant::now();
        s.undo().unwrap().unwrap();
        event(&mut events, trace, "undo-setup", 0, start);
        let initial = new_document(257, 259, Some(Rgba([37, 19, 83, 0])), 42, POLICY).unwrap();
        assert_eq!(s.document().state_id, initial.state_id);
        assert_eq!(s.undo_depth(), 0);
        let layer = &s.document().layers[&s.document().roots[0]];
        assert_eq!(layer.raster.resident_tiles(), 0);
        assert_eq!(layer.raster.get(256, 258), Rgba([37, 19, 83, 0]));
        let start = Instant::now();
        s.redo().unwrap().unwrap();
        event(&mut events, trace, "redo-setup", 0, start);
        assert_eq!(fingerprint(&s), expected[0]);
    }
    for (i, want) in expected.iter().enumerate().skip(1) {
        let start = Instant::now();
        s.redo().unwrap().unwrap();
        event(&mut events, trace, "redo", i, start);
        assert_eq!(fingerprint(&s), *want);
    }
    let start = Instant::now();
    drop(std::hint::black_box(s));
    event(&mut events, trace, "drop-history", 0, start);
    Outcome {
        before_release,
        after_release,
        events,
        rss_before_release_kib,
        rss_after_release_kib,
    }
}

fn rss_kib() -> Option<u64> {
    let pid = std::process::id().to_string();
    let output = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()
}
