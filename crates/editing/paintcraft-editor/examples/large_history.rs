#[path = "support/large_history.rs"]
#[allow(dead_code)]
mod replay;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("verify") => {
            let cases = args
                .get(1)
                .map(|s| vec![s.as_str()])
                .unwrap_or_else(|| replay::CASES.to_vec());
            for case in cases {
                for variant in replay::VARIANTS {
                    std::hint::black_box(replay::verify(case, variant));
                }
            }
            println!(
                "Complete recorded IDs/tile CRCs and all unchanged identities restored; no leases retained by fingerprints"
            );
        }
        Some("calibrate") => {
            std::hint::black_box(replay::fixture(&args[1]));
        }
        Some("cleanup") => {
            println!(
                "Removed {} abandoned stores",
                paintcraft_platform::spill::remove_abandoned(
                    &std::env::temp_dir().join("paintcraft-large-history-bench")
                )
                .unwrap()
            );
        }
        _ => {
            let outcome = replay::run(&args[1], &args[0], args.len() == 3);
            println!("memory={:?}\nstore={:?}", outcome.memory, outcome.store);
            if let Some(path) = args.get(2) {
                use std::io::Write;
                let mut file = std::fs::File::create_new(path).unwrap();
                writeln!(file, "phase,index,ns").unwrap();
                for (phase, n, ns) in outcome.events {
                    writeln!(file, "{phase},{n},{ns}").unwrap();
                }
            }
        }
    }
}
