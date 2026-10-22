//! Frozen executor versus whole-batch compilation, including result destruction.
#[path = "support/sequential.rs"]
#[allow(dead_code)]
mod sequential;
use criterion::{BenchmarkId, Criterion, SamplingMode};
use paintcraft_commands::Batch;
use paintcraft_core::types::{Rgba, resources::SharedBudget};
use paintcraft_document::{Document, POLICY, hex, import_image, new_document};
use paintcraft_editor::execute::{self, Selection};
use paintcraft_raster::Admission;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    hint::black_box,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;
const NAMES: [&str; 7] = [
    "horizontal_pencil",
    "diagonal_pencil",
    "smooth_brush",
    "smooth_eraser",
    "mixed_lineage",
    "default_control",
    "dense",
];
struct Case {
    name: &'static str,
    doc: Document,
    batch: Batch,
    repeats: usize,
}
#[derive(Clone, Copy)]
enum Mode {
    Sequential,
    Planned,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Sequential => "sequential",
            Self::Planned => "planned",
        }
    }
}
const MODES: [Mode; 2] = [Mode::Sequential, Mode::Planned];
fn load(root: &Path, only: Option<&str>) -> Vec<Case> {
    NAMES
        .into_iter()
        .filter(|name| only.is_none_or(|wanted| wanted == *name))
        .map(|name| {
            let doc = if name == "dense" {
                let bytes = std::fs::read(root.join("dense.rgba")).unwrap();
                let sha = std::fs::read_to_string(root.join("dense.sha256")).unwrap();
                import_image(1000, 1000, &bytes, sha.trim()).unwrap()
            } else {
                new_document(1000, 1000, Some(Rgba([255; 4])), 0, POLICY).unwrap()
            };
            let batch =
                Batch::from_json(&std::fs::read(root.join(format!("{name}.json"))).unwrap())
                    .unwrap();
            assert_eq!(batch.base_state, doc.state_id);
            Case {
                name,
                doc,
                batch,
                repeats: if name == "default_control" { 100 } else { 5 },
            }
        })
        .collect()
}
struct Output {
    doc: Document,
    selection: Selection,
    aliases: BTreeMap<String, Uuid>,
    decoded_peak: u64,
    metadata_peak: u64,
}
fn run(case: &Case, mode: Mode) -> Output {
    let decoded = SharedBudget::new(64 << 20);
    let metadata = SharedBudget::new(256 << 20);
    let admission = Admission::with_residency(execute::STAGED_PIXELS_MAX, decoded.clone())
        .with_paint_metadata_budget(metadata.clone());
    let (doc, selection, aliases) = match mode {
        Mode::Sequential => {
            let out = sequential::execute_with_admission(
                &case.doc,
                Selection::Inactive,
                &case.batch,
                admission,
            )
            .unwrap();
            (out.doc, out.selection, out.aliases)
        }
        Mode::Planned => {
            let out = execute::execute_with_admission(
                &case.doc,
                Selection::Inactive,
                &case.batch,
                admission,
            )
            .unwrap();
            (out.doc, out.selection, out.aliases)
        }
    };
    assert_eq!(metadata.usage().reserved, 0);
    Output {
        doc,
        selection,
        aliases,
        decoded_peak: decoded.usage().peak,
        metadata_peak: metadata.usage().peak,
    }
}
fn digest(doc: &Document) -> String {
    let mut hash = Sha256::new();
    hash.update(format!(
        "{} {} {:?} {:?}",
        doc.document_id, doc.state_id, doc.canvas, doc.title
    ));
    for layer in doc.layers_in_order() {
        hash.update(format!(
            "{} {} {:?} {} {} {} {} {:?}",
            layer.id,
            layer.name,
            layer.origin,
            layer.visible,
            layer.opacity,
            layer.raster.width(),
            layer.raster.height(),
            layer.raster.default_pixel()
        ));
        let mut row = vec![0u8; layer.raster.width() as usize * 4];
        for y in 0..i64::from(layer.raster.height()) {
            for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                pixel.copy_from_slice(&layer.raster.get(x as i64, y).0);
            }
            hash.update(&row);
        }
    }
    hex(&hash.finalize())
}
fn verify(cases: &[Case]) {
    for case in cases {
        let a = run(case, Mode::Sequential);
        let b = run(case, Mode::Planned);
        assert!(a.doc.same_semantics(&b.doc));
        assert_eq!(a.selection, b.selection);
        assert_eq!(a.aliases, b.aliases);
        assert_eq!(a.decoded_peak, b.decoded_peak);
        let checksum = digest(&a.doc);
        assert_eq!(checksum, digest(&b.doc));
        for (mode, out) in [(Mode::Sequential, a), (Mode::Planned, b)] {
            println!(
                "{} {} sha256={} decoded_peak={} metadata_peak={} operations={} repeats={}",
                case.name,
                mode.name(),
                checksum,
                out.decoded_peak,
                out.metadata_peak,
                case.batch.operations.len(),
                case.repeats
            );
        }
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "calibration") {
        return;
    }
    let root =
        PathBuf::from(std::env::var("PAINTCRAFT_BATCH_FIXTURES").expect("set fixture directory"));
    let cases = load(
        &root,
        if args.get(1).is_some_and(|s| s == "process") {
            args.get(2).map(String::as_str)
        } else {
            None
        },
    );
    if args.get(1).is_some_and(|s| s == "verify") {
        verify(&cases);
        return;
    }
    if args.get(1).is_some_and(|s| s == "process") {
        let case = cases
            .iter()
            .find(|case| Some(case.name) == args.get(2).map(String::as_str))
            .unwrap();
        let mode = MODES
            .into_iter()
            .find(|mode| Some(mode.name()) == args.get(3).map(String::as_str))
            .unwrap();
        for _ in 0..case.repeats {
            drop(black_box(run(black_box(case), black_box(mode))));
        }
        return;
    }
    verify(&cases);
    let mut criterion = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1))
        .configure_from_args();
    let mut group = criterion.benchmark_group("batch_preparation");
    group.sampling_mode(SamplingMode::Flat);
    for case in &cases {
        for mode in MODES {
            group.bench_with_input(
                BenchmarkId::new(case.name, mode.name()),
                &mode,
                |bench, &mode| {
                    bench.iter(|| drop(black_box(run(black_box(case), black_box(mode)))))
                },
            );
        }
    }
    group.finish();
    criterion.final_summary();
}
