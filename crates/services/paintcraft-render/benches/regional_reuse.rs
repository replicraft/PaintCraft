//! Complete edit/render replay across measured composite granularities.
#[path = "../examples/support/replay.rs"]
mod replay;
#[path = "../examples/support/mod.rs"]
#[allow(dead_code)]
mod support;
use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use std::{hint::black_box, time::Duration};

fn regional_reuse(c: &mut Criterion) {
    let mut group = c.benchmark_group("regional_reuse");
    group
        .sample_size(20)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(1));
    for case in [
        "sparse1",
        "sparse5",
        "dense5",
        "global-bottom5",
        "viewport100",
    ] {
        for variant in replay::VARIANTS {
            replay::Replay::verify(case, variant);
            group.bench_function(BenchmarkId::new(case, *variant), |b| {
                b.iter(|| {
                    let mut replay = replay::Replay::new(black_box(case), black_box(variant));
                    for _ in 0..8 {
                        drop(black_box(replay.step()));
                    }
                    drop(replay);
                })
            });
        }
    }
    group.finish();
}
criterion_group!(benches, regional_reuse);
criterion_main!(benches);
