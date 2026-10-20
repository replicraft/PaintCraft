#[path = "../examples/support/replay.rs"]
mod replay;
#[path = "../examples/support/mod.rs"]
#[allow(dead_code)]
mod support;
use replay::Replay;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();
fn main() {
    divan::main();
}

#[divan::bench(args=replay::CASES,consts=[0,64,128,256])]
fn edit_and_render<const EDGE: u32>(bencher: divan::Bencher, case: &str) {
    let variant = if EDGE == 0 {
        "full".to_owned()
    } else {
        format!("tile{EDGE}")
    };
    Replay::verify(case, &variant);
    let mut replay = Replay::new(case, &variant);
    bencher.bench_local(|| {
        divan::black_box_drop(replay.step());
    });
}
