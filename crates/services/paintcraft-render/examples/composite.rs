//! Process-level CPU composition driver, not part of the shipping CLI.
mod support;
use std::{hint::black_box, time::Instant};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "verify") {
        for case in support::CASES {
            support::verify(&support::fixture(case));
        }
        println!("Exact baseline/production equivalence passed for all fixtures");
        return;
    }
    assert_eq!(args.len(), 3, "variant case iterations");
    let doc = support::fixture(&args[1]);
    let iterations: usize = args[2].parse().unwrap();
    let start = Instant::now();
    for _ in 0..iterations {
        let pixels = match args[0].as_str() {
            "baseline" => support::reference::reference(black_box(&doc), || Ok(())),
            "production" => paintcraft_render::render_flat(black_box(&doc)),
            _ => panic!("unknown variant"),
        }
        .unwrap();
        black_box(pixels);
    }
    println!(
        "case={} iterations={iterations} render_ns={}",
        args[1],
        start.elapsed().as_nanos()
    );
}
