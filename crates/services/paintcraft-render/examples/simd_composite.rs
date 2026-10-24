//! BF02 process comparison. Includes fixture construction and output destruction.
#[path = "support/scalar_spans.rs"]
#[allow(dead_code)]
mod scalar_spans;
#[allow(dead_code)]
mod support;
use std::hint::black_box;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "verify") {
        for case in support::CASES {
            let doc = support::fixture(case);
            let expected = support::reference::reference(&doc, || Ok(())).unwrap();
            assert_eq!(expected, scalar_spans::render_flat(&doc).unwrap(), "{case}");
            assert_eq!(
                expected,
                paintcraft_render::render_flat(&doc).unwrap(),
                "{case}"
            );
        }
        println!("independent row-copy, frozen scalar spans and production match");
        return;
    }
    assert_eq!(args.len(), 3, "legacy|production case iterations");
    let doc = support::fixture(&args[1]);
    let iterations: usize = args[2].parse().unwrap();
    for _ in 0..iterations {
        let result = match args[0].as_str() {
            "legacy" => scalar_spans::render_flat(black_box(&doc)),
            "production" => paintcraft_render::render_flat(black_box(&doc)),
            _ => panic!("unknown variant"),
        }
        .unwrap();
        drop(black_box(result));
    }
}
