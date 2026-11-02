//! Process lifecycle: fixture setup, fresh jobs, complete outputs and destruction.
#[path = "support/layer_blend.rs"]
#[allow(dead_code)]
mod support;
use std::hint::black_box;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["verify"] {
        support::verify_all();
        return;
    }
    assert_eq!(args.len(), 4, "run|zero case opaque|general iterations");
    let case = support::case(&args[1]);
    let fixture = support::fixture(case, support::Variant::parse(&args[2]));
    let iterations: u32 = args[3].parse().unwrap();
    assert!((1..=32).contains(&iterations));
    for _ in 0..iterations {
        match args[0].as_str() {
            "run" => drop(black_box(support::render(black_box(&fixture)))),
            "zero" => {
                black_box(&fixture);
                support::calibrate();
            }
            _ => panic!("unknown action"),
        }
    }
}
