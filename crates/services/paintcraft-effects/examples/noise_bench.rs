//! Prebuilt process workload; compilation is never timed as raster work.
#[path = "support/noise_kernel.rs"]
mod workload;
use std::hint::black_box;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert!(
        (3..=4).contains(&args.len()),
        "verify|run|identity CASE [iterations]"
    );
    let case = workload::find(&args[2]);
    let amount = if args[1] == "identity" { 0 } else { 10 };
    assert!(["verify", "run", "identity"].contains(&args[1].as_str()));
    let iterations: usize = args
        .get(3)
        .map_or(1, |s| s.parse().expect("integer iterations"));
    assert!((1..=32).contains(&iterations));
    let mut frame = workload::Frame::new(case).unwrap();
    let mut observations = Vec::new();
    if args[1] == "verify" {
        for amount in [0, 10] {
            let used = frame.run(amount, true).unwrap();
            assert_eq!(used.counted_callbacks, frame.expected_callbacks());
            frame.verify(amount).unwrap();
            observations.push(serde_json::json!({"amount":amount,"actual_work":used.work,"actual_counted_callbacks":used.counted_callbacks,"expected_callbacks":frame.expected_callbacks(),"output_sha256":frame.digest()}));
        }
    } else {
        for _ in 0..iterations {
            let used = frame.run(black_box(amount), false).unwrap();
            assert_eq!(used.work, frame.quoted_work(amount));
        }
        frame.verify(amount).unwrap();
    }
    let (input_bytes, output_bytes) = frame.payload_bytes();
    println!(
        "{}",
        serde_json::json!({"scope":"Standalone kernel/process workload, not UI or algorithm ranking","case":case.id,"optional":case.optional,"width":case.width,"height":case.height,"amount":amount,"iterations":iterations,"quoted_work_per_frame":frame.quoted_work(amount),"source_period_payload_bytes":input_bytes,"full_output_payload_bytes":output_bytes,"source_period_sha256":frame.source_hash(),"actual_output_sha256":frame.digest(),"source_row_origin":"0,y","correctness_observations":observations,"observation_scope":"Counted verify-only correctness; run/identity timed modes leave observations empty"})
    );
}
