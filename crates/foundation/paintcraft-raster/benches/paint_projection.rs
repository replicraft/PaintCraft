//! Preparation plus exact flood classification; no native input/display claim.
use criterion::{BenchmarkId, Criterion, SamplingMode};
use paintcraft_core::types::{Rect, Rgba, resources::SharedBudget};
use paintcraft_raster::{
    Admission, BlendMode, PaintPlan, PaintProjection, RasterLayer, flood, shape::Span,
};
use std::{hint::black_box, time::Duration};

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    width: u32,
    height: u32,
    plans: usize,
    default: bool,
    repeats: usize,
}
const CASES: [Case; 4] = [
    Case {
        name: "tiny",
        width: 8,
        height: 8,
        plans: 1,
        default: false,
        repeats: 10000,
    },
    Case {
        name: "wall",
        width: 512,
        height: 64,
        plans: 1,
        default: false,
        repeats: 100,
    },
    Case {
        name: "overlap",
        width: 128,
        height: 64,
        plans: 64,
        default: false,
        repeats: 100,
    },
    Case {
        name: "default-control",
        width: 512,
        height: 64,
        plans: 1,
        default: true,
        repeats: 100,
    },
];
#[derive(Clone, Copy)]
enum Mode {
    Materialized,
    Projected,
    Combined,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Materialized => "materialized",
            Self::Projected => "projected",
            Self::Combined => "combined",
        }
    }
}
const MODES: [Mode; 3] = [Mode::Materialized, Mode::Projected, Mode::Combined];
const BACK: Rgba = Rgba([31, 63, 127, 0]);
fn plans(case: Case) -> Vec<PaintPlan> {
    (0..case.plans)
        .map(|i| {
            let mut plan = PaintPlan::default();
            let (area, color, mode) = if case.default {
                (
                    Rect::new(0, 0, i64::from(case.width), i64::from(case.height)).unwrap(),
                    BACK,
                    BlendMode::Replace,
                )
            } else if case.plans > 1 {
                (
                    Rect::new(
                        i64::from(case.width / 4),
                        0,
                        i64::from(case.width / 2),
                        i64::from(case.height),
                    )
                    .unwrap(),
                    Rgba([i as u8, 88, 156, 8]),
                    BlendMode::SourceOver,
                )
            } else {
                (
                    Rect::new(i64::from(case.width / 2), 0, 1, i64::from(case.height - 1)).unwrap(),
                    Rgba([17, 88, 156, 128]),
                    BlendMode::SourceOver,
                )
            };
            plan.rect(area, color, mode).unwrap();
            plan
        })
        .collect()
}
fn projections(case: Case, bounds: Rect) -> Vec<PaintProjection> {
    plans(case)
        .into_iter()
        .map(|plan| plan.into_projection_checked(bounds, &|| Ok(())).unwrap())
        .collect()
}
fn views(case: Case, bounds: Rect, mode: Mode) -> Vec<PaintProjection> {
    if matches!(mode, Mode::Projected) {
        return projections(case, bounds);
    }
    let mut plans = plans(case).into_iter();
    let mut program = plans
        .next()
        .expect("nonempty fixture")
        .into_projection_checked(bounds, &|| Ok(()))
        .unwrap();
    for plan in plans {
        program = program.append_checked(plan, &|| Ok(())).unwrap();
    }
    vec![program]
}
fn read(views: &[PaintProjection], x: i64, y: i64) -> Rgba {
    let mut pixel = BACK;
    for view in views {
        pixel = view
            .read_checked(x, y, |_, _| Ok(pixel), &|| Ok(()))
            .unwrap();
    }
    pixel
}
fn materialize(case: Case, admission: &mut Admission) -> RasterLayer {
    let mut layer = RasterLayer::new(case.width, case.height, BACK);
    for plan in plans(case) {
        plan.apply(&mut layer, admission).unwrap();
    }
    layer
}
struct Output {
    region: Vec<Span>,
    decoded_peak: u64,
    projection_payload: usize,
}
fn run(case: Case, mode: Mode) -> Output {
    let bounds = Rect::new(0, 0, i64::from(case.width), i64::from(case.height)).unwrap();
    if !matches!(mode, Mode::Materialized) {
        let views = views(case, bounds, mode);
        let projection_payload = views.iter().map(PaintProjection::metadata_bytes).sum();
        let region = flood::region_from_source_checked(
            |x, y| Ok(read(&views, x, y)),
            (0, 0),
            &bounds,
            0,
            64 << 20,
            &|| Ok(()),
        )
        .unwrap();
        Output {
            region,
            decoded_peak: 0,
            projection_payload,
        }
    } else {
        let budget = SharedBudget::new(16 << 20);
        let mut admission = Admission::with_residency(16 * 65_536, budget.clone());
        let layer = materialize(case, &mut admission);
        let region = flood::region(&layer, (0, 0), &bounds, 0).unwrap();
        Output {
            region,
            decoded_peak: budget.usage().peak,
            projection_payload: 0,
        }
    }
}
fn canonical(region: &mut [Span]) {
    region.sort_unstable_by_key(|s| (s.y, s.x0, s.x1));
}
fn verify() {
    for case in CASES {
        let bounds = Rect::new(0, 0, i64::from(case.width), i64::from(case.height)).unwrap();
        let layer = materialize(
            case,
            &mut Admission::with_residency(16 * 65_536, SharedBudget::new(16 << 20)),
        );
        let mut reference = run(case, Mode::Materialized);
        canonical(&mut reference.region);
        for mode in MODES {
            if !matches!(mode, Mode::Materialized) {
                let views = views(case, bounds, mode);
                for y in 0..i64::from(case.height) {
                    for x in 0..i64::from(case.width) {
                        assert_eq!(
                            read(&views, x, y),
                            layer.get(x, y),
                            "{} {} {x},{y}",
                            case.name,
                            mode.name()
                        );
                    }
                }
            }
            let mut out = run(case, mode);
            canonical(&mut out.region);
            assert_eq!(
                reference.region,
                out.region,
                "{} {}",
                case.name,
                mode.name()
            );
            println!(
                "{} {} decoded_peak={} projection_payload={} spans={} pixels={} repeats={}",
                case.name,
                mode.name(),
                out.decoded_peak,
                out.projection_payload,
                out.region.len(),
                out.region.iter().map(|s| s.x1 - s.x0 + 1).sum::<i64>(),
                case.repeats
            );
        }
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "verify") {
        verify();
        return;
    }
    if args.get(1).is_some_and(|a| a == "calibration") {
        return;
    }
    if args.get(1).is_some_and(|a| a == "process") {
        let case = CASES
            .into_iter()
            .find(|c| Some(c.name) == args.get(2).map(String::as_str))
            .expect("known case");
        let mode = MODES
            .into_iter()
            .find(|mode| Some(mode.name()) == args.get(3).map(String::as_str))
            .expect("known implementation");
        for _ in 0..case.repeats {
            drop(black_box(run(black_box(case), black_box(mode))));
        }
        return;
    }
    verify();
    let mut criterion = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_secs(1))
        .configure_from_args();
    let mut group = criterion.benchmark_group("paint_projection");
    group.sampling_mode(SamplingMode::Flat);
    for case in CASES {
        for mode in MODES {
            group.bench_with_input(
                BenchmarkId::new(case.name, mode.name()),
                &mode,
                |bench, &mode| {
                    bench.iter(|| drop(black_box(run(black_box(case), black_box(mode)))))
                },
            );
        }
    }
    group.finish();
    criterion.final_summary();
}
