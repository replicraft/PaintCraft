//! Matched immutable-input/private-output Effects rows; separately priced real CPU jobs.
#[path = "color_overlay_product/support.rs"]
mod support;
use criterion::{
    BenchmarkId, Criterion, SamplingMode, Throughput, criterion_group, criterion_main,
};
use paintcraft_core::types::{Rgba, json::Value};
use paintcraft_effects::color_overlay::{self, prepared::PreparedColorOverlay};
use std::{cell::Cell, hint::black_box, time::Duration};
fn run(
    source: &[Rgba],
    output: &mut [Rgba],
    policy: paintcraft_core::types::styles::ColorOverlayRgbV1,
    prepared: bool,
) -> u64 {
    // Required immutable-source copy is timed identically in both variants.
    output.copy_from_slice(source);
    let calls = Cell::new(0_u64);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    if source.is_empty() {
        if prepared {
            PreparedColorOverlay::new(policy)
                .pixels_in_place(output, &check)
                .unwrap()
        } else {
            color_overlay::evaluate_row(policy, source, output, &check).unwrap()
        }
    }
    for (input, out) in source.chunks(64).zip(output.chunks_mut(64)) {
        if prepared {
            PreparedColorOverlay::new(policy)
                .pixels_in_place(out, &check)
                .unwrap()
        } else {
            color_overlay::evaluate_row(policy, input, out, &check).unwrap()
        }
    }
    calls.get()
}
fn compare(c: &mut Criterion) {
    let manifest = support::manifest();
    let reverse = match std::env::var("PAINTCRAFT_COLOR_OVERLAY_BENCH_ORDER").as_deref() {
        Ok("reverse") => true,
        Ok("forward") | Err(_) => false,
        _ => panic!("order must be forward/reverse"),
    };
    let mut rows: Vec<&Value> = manifest
        .get("kernel")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .collect();
    if reverse {
        rows.reverse()
    }
    let mut group = c.benchmark_group("color_overlay_matched_effects_rows_unpriced");
    group
        .sample_size(30)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for row in rows {
        let n = support::number(row, "pixels") as usize;
        let source = support::pixels(n);
        let policy = support::policy(
            support::number(row, "mode") as u8,
            support::number(row, "opacity") as u8,
        );
        assert_eq!(
            support::hash(&source),
            support::expected(row, "source_fnv64")
        );
        let mut scalar = vec![Rgba([0; 4]); n];
        let mut prepared = scalar.clone();
        assert_eq!(
            run(&source, &mut scalar, policy, false),
            support::number(row, "callbacks")
        );
        assert_eq!(
            run(&source, &mut prepared, policy, true),
            support::number(row, "callbacks")
        );
        assert_eq!(scalar, prepared);
        assert_eq!(
            support::hash(&scalar),
            support::expected(row, "output_fnv64")
        );
        let name = format!(
            "mode{}-n{n}-opacity{}",
            support::number(row, "mode"),
            support::number(row, "opacity")
        );
        eprintln!("preflight {name} fnv64={:016x}", support::hash(&scalar));
        group.throughput(Throughput::Elements(n as u64));
        for variant in if reverse {
            [true, false]
        } else {
            [false, true]
        } {
            let mut output = vec![Rgba([0; 4]); n];
            group.bench_function(
                BenchmarkId::new(
                    &name,
                    if variant {
                        "prepared_per64_copy_callbacks"
                    } else {
                        "original_scalar_row_per64_copy_callbacks"
                    },
                ),
                |b| {
                    b.iter(|| {
                        let count = run(
                            black_box(&source),
                            black_box(&mut output),
                            black_box(policy),
                            variant,
                        );
                        black_box(count);
                        black_box(&output);
                    });
                },
            );
            assert_eq!(
                support::hash(&output),
                support::expected(row, "output_fnv64")
            );
        }
    }
    group.finish();
    let mut rows: Vec<&Value> = manifest
        .get("renderer")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .collect();
    if reverse {
        rows.reverse()
    }
    let mut group = c.benchmark_group("color_overlay_actual_cpu_original_admission");
    group
        .sample_size(30)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for row in rows {
        let doc = support::document(row);
        let before = doc.clone();
        let (rgba, work, callbacks) = support::render(&doc);
        assert_eq!(
            support::hash_bytes(&rgba),
            support::expected(row, "output_fnv64")
        );
        assert!(doc.same_semantics(&before));
        let name = format!(
            "mode{}-{}x{}-{}",
            support::number(row, "mode"),
            doc.canvas.0,
            doc.canvas.1,
            if row.get("full_pipeline").unwrap().as_bool().unwrap() {
                "mixed-filter-mask-style-ownmask-opacity-multiply"
            } else {
                "uniform-style"
            }
        );
        eprintln!(
            "preflight {name} original_work={work} callbacks={callbacks} fnv64={:016x}",
            support::hash_bytes(&rgba)
        );
        group.throughput(Throughput::Elements(
            u64::from(doc.canvas.0) * u64::from(doc.canvas.1),
        ));
        group.bench_function(
            BenchmarkId::new(name, "cold_original_factory_render_output_teardown"),
            |b| {
                b.iter(|| {
                    // Independent completed jobs each construct/drop their own original
                    // caller. No failed/in-flight job is renewed. Construction is timed.
                    let result = support::render(black_box(&doc));
                    black_box(result);
                })
            },
        );
        let (rgba, _, _) = support::render(&doc);
        assert_eq!(
            support::hash_bytes(&rgba),
            support::expected(row, "output_fnv64")
        );
        assert!(doc.same_semantics(&before));
    }
    group.finish();
    c.bench_function(
        "color_overlay_original_factory_tls_teardown_calibration",
        |b| b.iter(support::calibrate),
    );
}
criterion_group!(benches, compare);
criterion_main!(benches);
