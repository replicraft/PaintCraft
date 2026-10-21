//! Shared-code comparison: planned replay versus direct incremental raster APIs.
use criterion::{BenchmarkId, Criterion};
use paintcraft_core::types::{Rect, Rgba};
use paintcraft_raster::{Admission, BlendMode, PaintPlan, RasterLayer};
use std::{hint::black_box, time::Duration};
#[derive(Clone, Copy)]
enum Write {
    Pixel(i64, i64),
    Run(Rect, u8),
}
struct Case {
    name: &'static str,
    base: RasterLayer,
    writes: Vec<Write>,
}
fn cases() -> Vec<Case> {
    let base = RasterLayer::new(1000, 1000, Rgba([255; 4]));
    vec![
        Case {
            name: "horizontal_pencil",
            base: base.clone(),
            writes: (0..1000).map(|x| Write::Pixel(x, 400)).collect(),
        },
        Case {
            name: "diagonal_pencil",
            base: base.clone(),
            writes: (0..1000).map(|x| Write::Pixel(x, x)).collect(),
        },
        Case {
            name: "brush_runs",
            base: base.clone(),
            writes: (400..432)
                .map(|y| Write::Run(Rect::new(0, y, 1000, 1).unwrap(), 16))
                .collect(),
        },
        Case {
            name: "partial_eraser_runs",
            base,
            writes: (400..432)
                .map(|y| Write::Run(Rect::new(0, y, 1000, 1).unwrap(), 7))
                .collect(),
        },
    ]
}
fn run(case: &Case, planned: bool) -> RasterLayer {
    let color = Rgba([17, 88, 156, 128]);
    let mut layer = case.base.clone();
    let mut admission = Admission::new(16 * 65_536);
    if planned {
        let mut plan = PaintPlan::default();
        for write in &case.writes {
            match *write {
                Write::Pixel(x, y) => plan
                    .rect(Rect::new(x, y, 1, 1).unwrap(), color, BlendMode::SourceOver)
                    .unwrap(),
                Write::Run(area, 16) => plan.rect(area, color, BlendMode::SourceOver).unwrap(),
                Write::Run(area, samples) => plan.coverage_rect(area, color, samples).unwrap(),
            }
        }
        plan.apply(&mut layer, &mut admission).unwrap();
    } else {
        for write in &case.writes {
            match *write {
                Write::Pixel(x, y) => layer
                    .blend_pixel(x, y, color, BlendMode::SourceOver, &mut admission)
                    .unwrap(),
                Write::Run(area, 16) => layer
                    .fill_rect(&area, color, BlendMode::SourceOver, &mut admission)
                    .unwrap(),
                Write::Run(area, samples) => {
                    for y in area.y0..area.y1 {
                        for x in area.x0..area.x1 {
                            let dst = layer.get(x, y);
                            let pixel = Rgba(std::array::from_fn(|i| {
                                ((u32::from(color.0[i]) * u32::from(samples)
                                    + u32::from(dst.0[i]) * u32::from(16 - samples)
                                    + 8)
                                    / 16) as u8
                            }));
                            layer
                                .blend_pixel(x, y, pixel, BlendMode::Replace, &mut admission)
                                .unwrap();
                        }
                    }
                }
            }
        }
    }
    black_box(&layer);
    layer
}
fn main() {
    let cases = cases();
    for case in &cases {
        assert_eq!(run(case, false), run(case, true), "{}", case.name);
    }
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "process") {
        let case = cases
            .iter()
            .find(|case| Some(case.name) == args.get(2).map(String::as_str))
            .expect("known case");
        let planned = match args.get(3).map(String::as_str) {
            Some("planned") => true,
            Some("incremental") => false,
            _ => panic!("known implementation"),
        };
        for _ in 0..100 {
            drop(black_box(run(case, planned)));
        }
        return;
    }
    let mut criterion = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(1))
        .configure_from_args();
    let mut group = criterion.benchmark_group("paint_plan");
    for case in &cases {
        for planned in [false, true] {
            group.bench_with_input(
                BenchmarkId::new(case.name, if planned { "planned" } else { "incremental" }),
                &planned,
                |bench, &planned| {
                    bench.iter(|| {
                        drop(black_box(run(black_box(case), planned)));
                    })
                },
            );
        }
    }
    group.finish();
    criterion.final_summary();
}
