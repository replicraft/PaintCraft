//! Quantify isolated exact LUT construction; not UI or whole-render throughput.
use paintcraft_core::types::filters::LevelsRgbV1;
use paintcraft_effects::levels::{Limits, Scratch, build_lut, quote};
use sha2::{Digest, Sha256};
use std::{hint::black_box, time::Instant};

fn main() {
    let iterations: usize = std::env::var("PAINTCRAFT_LEVELS_BENCH_ITERATIONS")
        .map_or(Ok(15), |value| value.parse())
        .expect("iterations must be an integer");
    assert!((1..=64).contains(&iterations), "iterations must be 1..64");
    let cases = [
        ("identity", [0, 255, 100, 0, 255]),
        ("linear-compression", [64, 192, 100, 16, 240]),
        ("square", [0, 255, 50, 0, 255]),
        ("square-root", [0, 255, 200, 0, 255]),
        ("gamma-min", [0, 255, 10, 0, 255]),
        ("gamma-near-min", [0, 255, 11, 0, 255]),
        ("gamma-near-max", [0, 255, 997, 0, 255]),
        ("gamma-max", [0, 255, 999, 0, 255]),
        ("inverted", [17, 201, 333, 240, 16]),
        ("constant", [0, 255, 999, 73, 73]),
        ("single-input-step", [127, 128, 999, 0, 255]),
    ];
    let mut scratch = Scratch::default();
    let mut table = [0u8; 256];
    let check = || Ok(());
    for (name, values) in cases {
        let [ib, iw, gamma, ob, ow] = values;
        let policy = LevelsRgbV1::new(ib, iw, gamma, ob, ow).unwrap();
        let required = quote(policy);
        let limits = Limits::from(required);
        // Reuse the caller's scratch; warm once, then time only construction.
        let warm = build_lut(policy, &mut scratch, &mut table, limits, &check).unwrap();
        let digest = Sha256::digest(table);
        let mut times = [0u64; 64];
        for sample in &mut times[..iterations] {
            let start = Instant::now();
            let used = build_lut(
                black_box(policy),
                black_box(&mut scratch),
                black_box(&mut table),
                limits,
                &check,
            )
            .unwrap();
            *sample = start.elapsed().as_nanos().try_into().unwrap();
            assert_eq!(used, warm, "work counts must be deterministic");
            black_box(&table);
        }
        assert_eq!(Sha256::digest(table), digest);
        times[..iterations].sort_unstable();
        let p50 = times[(iterations - 1) / 2];
        let p95 = times[(95 * iterations).div_ceil(100) - 1];
        let digest_hex = paintcraft_core::types::policy::hex(&digest);
        println!(
            concat!(
                "{{\"case\":\"{}\",\"parameters\":[{},{},{},{},{}],",
                "\"iterations\":{},\"scratch_reused\":true,",
                "\"scratch_bytes\":{},\"quoted_work\":{},\"actual_work\":{},",
                "\"nanoseconds\":{{\"min\":{},\"p50\":{},\"p95\":{},\"max\":{}}},",
                "\"lut_sha256\":\"{}\"}}"
            ),
            name,
            ib,
            iw,
            gamma,
            ob,
            ow,
            iterations,
            required.scratch_bytes,
            required.work,
            warm.units,
            times[0],
            p50,
            p95,
            times[iterations - 1],
            digest_hex,
        );
    }
}
