//! Calibrated raw statistical samples with the production system allocator.
#[path = "palette_assets/fixtures.rs"]
mod support;
use criterion::{
    BenchmarkId, Criterion, SamplingMode, Throughput, criterion_group, criterion_main,
};
use paintcraft_formats::ase;
use std::{hint::black_box, time::Duration};

fn palettes(c: &mut Criterion) {
    support::fixtures();
    support::manifest();
    let mut group = c.benchmark_group("ase");
    group
        .sample_size(100)
        .sampling_mode(SamplingMode::Flat)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    let mut cases = support::CASES.to_vec();
    if std::env::var_os("PAINTCRAFT_PALETTE_REVERSE").is_some() {
        cases.reverse();
    }
    for case in cases {
        let fixture = support::fixture(case);
        group.throughput(Throughput::Bytes(fixture.bytes.len() as u64));
        group.bench_function(BenchmarkId::new("decode", case), |b| {
            b.iter(|| {
                drop(black_box(
                    ase::decode(black_box(&fixture.bytes), &|| Ok(())).unwrap(),
                ));
            })
        });
        group.bench_function(BenchmarkId::new("encode", case), |b| {
            b.iter(|| {
                drop(black_box(
                    ase::encode(black_box(&fixture.palette), &|| Ok(())).unwrap(),
                ));
            })
        });
    }
    group.finish();
}
criterion_group!(benches, palettes);
criterion_main!(benches);
