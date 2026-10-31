//! Existing Criterion raw samples. Caller construction/probes/hash are untimed.
#[path = "../examples/support/noise_kernel.rs"]
#[allow(dead_code)]
mod workload;
use criterion::{BenchmarkId, Criterion, SamplingMode, criterion_group, criterion_main};
use std::{hint::black_box, time::Duration};
fn noise(c: &mut Criterion) {
    let mut group = c.benchmark_group("noise-kernel");
    group
        .sample_size(100)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    let optional = std::env::var_os("PAINTCRAFT_NOISE_EXACT2MP").is_some();
    let selected = std::env::var("PAINTCRAFT_NOISE_CASE").ok();
    let selected_amount = std::env::var("PAINTCRAFT_NOISE_AMOUNT")
        .ok()
        .map(|v| v.parse::<i16>().expect("0 or10 amount"));
    assert!(selected_amount.is_none_or(|v| [0, 10].contains(&v)));
    for case in workload::cases().filter(|case| {
        (optional || !case.optional) && selected.as_ref().is_none_or(|name| name == case.id)
    }) {
        for amount in [10, 0]
            .into_iter()
            .filter(|a| selected_amount.is_none_or(|v| v == *a))
        {
            let mut frame = workload::Frame::new(case).unwrap();
            let used = frame.run(amount, true).unwrap();
            assert_eq!(used.counted_callbacks, frame.expected_callbacks());
            frame.verify(amount).unwrap();
            group.bench_function(
                BenchmarkId::new(
                    case.id,
                    if amount == 0 {
                        "identity-calibration"
                    } else {
                        "default10"
                    },
                ),
                |b| {
                    b.iter(|| {
                        let used = frame.run(black_box(amount), false).unwrap();
                        black_box(used.work);
                    })
                },
            );
            frame.verify(amount).unwrap();
        }
    }
    group.finish();
}
criterion_group!(benches, noise);
criterion_main!(benches);
