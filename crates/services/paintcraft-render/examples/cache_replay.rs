// The shared fixture module also contains the older comparison driver helpers.
#[path = "support/replay.rs"]
mod replay;
#[allow(dead_code)]
mod support;
use replay::{CASES, Replay, VARIANTS};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "verify") {
        for case in CASES {
            for variant in VARIANTS {
                Replay::verify(case, variant);
            }
        }
        println!("All replayed transaction outputs match exact full regional composition");
        return;
    }
    assert_eq!(args.len(), 3, "variant case iterations");
    let mut replay = Replay::new(&args[1], &args[0]);
    let iterations: usize = args[2].parse().unwrap();
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        std::hint::black_box(replay.step());
    }
    println!(
        "iterations={iterations} edit_render_ns={}",
        start.elapsed().as_nanos()
    );
}
