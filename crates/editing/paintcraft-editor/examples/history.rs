#[path = "support/history.rs"]
#[allow(dead_code)] // Shared replay helper has several benchmark consumers.
mod replay;
use replay::{CASES, VARIANTS};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "verify") {
        let cases = args
            .get(1)
            .map(|s| vec![s.as_str()])
            .unwrap_or_else(|| CASES.to_vec());
        for case in cases {
            for variant in VARIANTS {
                std::hint::black_box(replay::run(case, variant, false));
            }
        }
        println!(
            "Exact recorded state IDs, tile CRCs and opacity restored for every retained step"
        );
        return;
    }
    if args.first().is_some_and(|s| s == "calibrate") {
        std::hint::black_box(replay::fixture(&args[1]));
        return;
    }
    assert!(args.len() >= 2, "variant case [trace.csv]");
    let out = replay::run(&args[1], &args[0], args.len() == 3);
    println!(
        "before_release={:?}\nafter_release={:?}",
        out.before_release, out.after_release
    );
    println!(
        "rss_before_release_kib={:?} rss_after_release_kib={:?}",
        out.rss_before_release_kib, out.rss_after_release_kib
    );
    if let Some(path) = args.get(2) {
        use std::io::Write;
        let mut file = std::fs::File::create_new(path).unwrap();
        writeln!(file, "phase,index,ns").unwrap();
        for event in out.events {
            writeln!(file, "{},{},{}", event.phase, event.index, event.ns).unwrap();
        }
    }
}
