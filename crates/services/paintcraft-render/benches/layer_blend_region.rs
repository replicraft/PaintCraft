//! Complete actual CPU region rendering; input setup/verification outside timing.
#[path = "../examples/support/layer_blend.rs"]
#[allow(dead_code)]
mod support;
use criterion::{
    BenchmarkId, Criterion, SamplingMode, Throughput, criterion_group, criterion_main,
};
use std::{hint::black_box, time::Duration};

fn regions(c: &mut Criterion) {
    // Verify ALL output bytes for both contenders before registering any timing.
    support::verify_all();
    let reverse = match std::env::var("PAINTCRAFT_BLEND_ORDER").as_deref() {
        Ok("reverse") => true,
        Ok("forward") | Err(_) => false,
        _ => panic!("unknown order"),
    };
    let mut cases = support::CASES;
    let mut variants = [support::Variant::Opaque, support::Variant::General];
    if reverse {
        cases.reverse();
        variants.reverse();
    }
    let mut group = c.benchmark_group("layer_blend_region");
    group
        .sample_size(100)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for case in cases {
        group.throughput(Throughput::Elements(
            u64::from(case.width) * u64::from(case.height),
        ));
        for variant in variants {
            let fixture = support::fixture(case, variant);
            group.bench_function(BenchmarkId::new(case.id, variant.name()), |b| {
                b.iter(|| {
                    drop(black_box(support::render(black_box(&fixture))));
                })
            });
        }
    }
    group.finish();
    let mut calibration = c.benchmark_group("layer_blend_region_zero");
    calibration
        .sample_size(100)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    calibration.bench_function("caller_factory_only", |b| b.iter(support::calibrate));
    calibration.finish();
}
criterion_group!(benches, regions);
criterion_main!(benches);
