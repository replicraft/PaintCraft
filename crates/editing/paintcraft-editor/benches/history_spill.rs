#[path = "../examples/support/history.rs"]
#[allow(dead_code)]
mod replay;
use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use std::time::Duration;
fn history(c: &mut Criterion) {
    let mut group = c.benchmark_group("history-spill");
    group
        .sample_size(20)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for case in replay::SPILL_CASES {
        for variant in replay::SPILL_VARIANTS {
            group.bench_function(BenchmarkId::new(*case, *variant), |b| {
                b.iter(|| {
                    drop(std::hint::black_box(replay::run_spill(
                        case, variant, false,
                    )))
                })
            });
        }
    }
    group.finish();
}
criterion_group!(benches, history);
criterion_main!(benches);
