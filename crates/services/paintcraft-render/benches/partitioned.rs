#[path = "../examples/support/partitioned.rs"]
mod partitioned;
#[path = "../examples/support/mod.rs"]
#[allow(dead_code)]
mod support;
use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use partitioned::Runner;
use std::{hint::black_box, time::Duration};

fn partitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("partitions");
    group
        .sample_size(100)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for case in partitioned::CASES {
        Runner::verify(case);
        let doc = support::fixture(case);
        let guard = partitioned::Guard::new(&doc);
        for variant in partitioned::VARIANTS {
            let runner = Runner::new(variant);
            group.bench_function(BenchmarkId::new(*case, *variant), |b| {
                b.iter(|| {
                    let pixels = runner.render(black_box(&doc));
                    guard.check(&pixels);
                    drop(black_box(pixels));
                })
            });
        }
    }
    group.finish();
}
criterion_group!(benches, partitions);
criterion_main!(benches);
