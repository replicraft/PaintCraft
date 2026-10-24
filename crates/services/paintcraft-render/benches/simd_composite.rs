//! BF02 steady-state complete composition, including returned-buffer destruction.
#[path = "../examples/support/scalar_spans.rs"]
#[allow(dead_code)]
mod scalar_spans;
#[path = "../examples/support/mod.rs"]
#[allow(dead_code)]
mod support;
use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use std::{hint::black_box, time::Duration};

fn composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_composite");
    group
        .sample_size(20)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(1));
    for case in [
        "dense-opaque5",
        "dense-opaque-odd5",
        "short-opaque5",
        "dense5",
        "sparse5",
        "uniform5",
    ] {
        let doc = support::fixture(case);
        support::verify(&doc);
        assert_eq!(
            scalar_spans::render_flat(&doc).unwrap(),
            paintcraft_render::render_flat(&doc).unwrap()
        );
        let candidate = if cfg!(feature = "simd-u16") {
            "wide-u16"
        } else if cfg!(feature = "simd-channels") {
            "wide-channels"
        } else if cfg!(feature = "simd") {
            "wide-packed"
        } else {
            "scalar-specialized"
        };
        for variant in ["legacy", candidate] {
            group.bench_function(BenchmarkId::new(case, variant), |b| {
                b.iter(|| {
                    let result = if variant == "legacy" {
                        scalar_spans::render_flat(black_box(&doc))
                    } else {
                        paintcraft_render::render_flat(black_box(&doc))
                    }
                    .unwrap();
                    drop(black_box(result));
                });
            });
        }
    }
    group.finish();
}
criterion_group!(benches, composition);
criterion_main!(benches);
