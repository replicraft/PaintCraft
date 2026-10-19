#[path = "../examples/support/mod.rs"]
mod support;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

#[divan::bench(args = support::CASES)]
fn baseline(bencher: divan::Bencher, case: &str) {
    let doc = support::fixture(case);
    support::verify(&doc);
    bencher
        .bench_local(|| support::reference::reference(divan::black_box(&doc), || Ok(())).unwrap());
}

#[divan::bench(args = support::CASES)]
fn production(bencher: divan::Bencher, case: &str) {
    let doc = support::fixture(case);
    support::verify(&doc);
    bencher.bench_local(|| paintcraft_render::render_flat(divan::black_box(&doc)).unwrap());
}
