//! Actual scalar API versus validated row API; no timings are source claims.
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use paintcraft_core::types::Rgba;
use paintcraft_render::display::DisplayAccumulator;
use std::{hint::black_box, time::Duration};

struct Case {
    name: &'static str,
    width: usize,
    height: usize,
    step: usize,
    rgba: Vec<u8>,
}
impl Case {
    fn new(name: &'static str, width: usize, height: usize, step: usize) -> Self {
        let mut rgba = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&[
                    x as u8,
                    (x * 3 + y) as u8,
                    (255usize.wrapping_sub(x)) as u8,
                    [0, 64, 128, 255][(x + y) % 4],
                ]);
            }
        }
        Self {
            name,
            width,
            height,
            step,
            rgba,
        }
    }
    fn cells(&self) -> usize {
        self.width.div_ceil(self.step) * self.height.div_ceil(self.step)
    }
    fn scalar(&self, dest: &mut [DisplayAccumulator]) {
        let width = self.width.div_ceil(self.step);
        for y in 0..self.height {
            for x in 0..self.width {
                let i = (y * self.width + x) * 4;
                dest[(y / self.step) * width + x / self.step]
                    .add(Rgba(self.rgba[i..i + 4].try_into().unwrap()))
                    .unwrap();
            }
        }
    }
    fn rows(&self, dest: &mut [DisplayAccumulator]) {
        let width = self.width.div_ceil(self.step);
        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                let n = (self.width - x).min(64);
                let start = (y / self.step) * width + x / self.step;
                let count = ((x % self.step) + n - 1) / self.step + 1;
                let i = (y * self.width + x) * 4;
                DisplayAccumulator::add_native_row(
                    &mut dest[start..start + count],
                    &self.rgba[i..i + n * 4],
                    self.step as u32,
                    (x % self.step) as u32,
                )
                .unwrap();
                x += n;
            }
        }
    }
    fn verify(&self) {
        let mut scalar = vec![DisplayAccumulator::default(); self.cells()];
        let mut rows = scalar.clone();
        self.scalar(&mut scalar);
        self.rows(&mut rows);
        for (a, b) in scalar.iter().zip(&rows) {
            assert_eq!(a.samples(), b.samples());
            assert_eq!(a.resolve().unwrap(), b.resolve().unwrap());
        }
    }
}

fn compare(c: &mut Criterion) {
    let mut cases = [
        Case::new("step1", 65, 3, 1),
        Case::new("step2", 65, 3, 2),
        Case::new("step16", 65, 3, 16),
        Case::new("step64", 65, 3, 64),
        Case::new("step128", 129, 3, 128),
        Case::new("default-native-field", 1000, 486, 16),
    ];
    let reverse = match std::env::var("PAINTCRAFT_ROW_BENCH_ORDER").as_deref() {
        Ok("reverse") => true,
        Ok("forward") | Err(_) => false,
        _ => panic!("unknown row benchmark order"),
    };
    if reverse {
        cases.reverse();
    }
    for case in &cases {
        case.verify();
    }
    let mut group = c.benchmark_group("display_native_rows");
    group
        .sample_size(30)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    for case in &cases {
        group.throughput(Throughput::Elements((case.width * case.height) as u64));
        for row in if reverse {
            [true, false]
        } else {
            [false, true]
        } {
            group.bench_function(
                BenchmarkId::new(case.name, if row { "validated_row" } else { "scalar_add" }),
                |b| {
                    b.iter_batched(
                        || vec![DisplayAccumulator::default(); case.cells()],
                        |mut cells| {
                            let input = black_box(case);
                            if row {
                                input.rows(black_box(&mut cells));
                            } else {
                                input.scalar(black_box(&mut cells));
                            }
                            black_box(cells)
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }
    group.finish();
}
criterion_group!(benches, compare);
criterion_main!(benches);
