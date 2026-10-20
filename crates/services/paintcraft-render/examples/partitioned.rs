#[path = "support/partitioned.rs"]
mod partitioned;
#[allow(dead_code)]
mod support;
use partitioned::{CASES, Runner};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "verify") {
        for case in args
            .get(1)
            .map(|case| vec![case.as_str()])
            .unwrap_or_else(|| CASES.to_vec())
        {
            Runner::verify(case);
        }
        println!("All bounded partition variants match the scalar reference");
        return;
    }
    assert_eq!(args.len(), 3, "variant case iterations");
    let doc = support::fixture(&args[1]);
    let runner = Runner::new(&args[0]);
    let guard = partitioned::Guard::new(&doc);
    let start = std::time::Instant::now();
    for _ in 0..args[2].parse::<usize>().unwrap() {
        let pixels = runner.render(&doc);
        guard.check(&pixels);
        std::hint::black_box(pixels);
    }
    println!("render_ns={}", start.elapsed().as_nanos());
}
