//! Allocation-profiled ASE workloads and supplemental unadjusted batches.
use paintcraft_formats::ase;
use std::{fmt::Write as _, time::Instant};
#[path = "palette_assets/fixtures.rs"]
mod support;
use support::{CASES, fixture, fixtures, manifest, publish};
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();
/// Supplemental raw wall-clock batches. Divan separately supplies calibrated
/// statistics/allocation counters. Setup is excluded; every result is dropped.
fn raw_samples() {
    let Ok(path) = std::env::var("PAINTCRAFT_PALETTE_SAMPLES") else {
        return;
    };
    let mut rows = String::from("case,operation,sample,iterations,elapsed_ns\n");
    let mut ordered: Vec<_> = fixtures().iter().collect();
    if std::env::var_os("PAINTCRAFT_PALETTE_REVERSE").is_some() {
        ordered.reverse();
    }
    for f in ordered {
        for operation in ["decode", "encode"] {
            for sample in 0..33 {
                let started = Instant::now();
                for _ in 0..16 {
                    if operation == "decode" {
                        divan::black_box_drop(
                            ase::decode(divan::black_box(&f.bytes), &|| Ok(())).unwrap(),
                        );
                    } else {
                        divan::black_box_drop(
                            ase::encode(divan::black_box(&f.palette), &|| Ok(())).unwrap(),
                        );
                    }
                }
                let ns = started.elapsed().as_nanos();
                if sample >= 3 {
                    writeln!(rows, "{},{},{},16,{}", f.name, operation, sample - 3, ns).unwrap();
                }
            }
        }
    }
    publish(std::path::Path::new(&path), rows.as_bytes());
}
fn main() {
    fixtures();
    manifest();
    raw_samples();
    divan::main();
}
#[divan::bench(args=CASES)]
fn decode(bencher: divan::Bencher, case: &str) {
    let f = fixture(case);
    bencher
        .counter(divan::counter::BytesCount::new(f.bytes.len()))
        .bench_local(|| {
            divan::black_box_drop(ase::decode(divan::black_box(&f.bytes), &|| Ok(())).unwrap());
        });
}
#[divan::bench(args=CASES)]
fn encode(bencher: divan::Bencher, case: &str) {
    let f = fixture(case);
    bencher
        .counter(divan::counter::BytesCount::new(f.bytes.len()))
        .bench_local(|| {
            divan::black_box_drop(ase::encode(divan::black_box(&f.palette), &|| Ok(())).unwrap());
        });
}
