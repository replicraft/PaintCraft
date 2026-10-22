//! Equivalent coverage output; setup outside timing, union construction/drop inside.
use criterion::{BenchmarkId, Criterion, SamplingMode};
use paintcraft_core::types::Rect;
use paintcraft_raster::shape::{CheckedSpanSet, DEFAULT_STROKE_SCRATCH_BYTES, Span, SpanSet};
use std::{collections::BTreeSet, hint::black_box, time::Duration};
struct Case {
    name: &'static str,
    n: i64,
    round: bool,
    points: Vec<(i64, i64)>,
}
fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "tiny",
            n: 1,
            round: false,
            points: (0..8).map(|x| (x, 8)).collect(),
        },
        Case {
            name: "square",
            n: 16,
            round: false,
            points: (0..512).map(|x| (x, 32)).collect(),
        },
        Case {
            name: "round",
            n: 31,
            round: true,
            points: (0..512).map(|x| (x, 32)).collect(),
        },
        Case {
            name: "retrace",
            n: 16,
            round: true,
            points: (0..4096).map(|i| ((i * 37) % 512, 32)).collect(),
        },
    ]
}
fn run(case: &Case, checked: bool) -> Vec<Span> {
    let clip = Rect::new(0, 0, 512, 64).unwrap();
    if checked {
        let mut set = CheckedSpanSet::with_scratch_limit(DEFAULT_STROKE_SCRATCH_BYTES);
        for &(x, y) in &case.points {
            set.add_stamp(x, y, case.n, case.round, &clip, &|| Ok(()))
                .unwrap();
        }
        let mut out = Vec::new();
        set.visit_spans(&|| Ok(()), |s| {
            out.push(s);
            Ok(())
        })
        .unwrap();
        out
    } else {
        let mut set = SpanSet::default();
        for &(x, y) in &case.points {
            set.add_stamp(x, y, case.n, case.round, &clip);
        }
        set.into_spans()
    }
}
fn pixels(spans: Vec<Span>) -> BTreeSet<(i64, i64)> {
    spans
        .iter()
        .flat_map(|s| (s.x0..=s.x1).map(move |x| (x, s.y)))
        .collect()
}
fn main() {
    let cases = cases();
    for case in &cases {
        assert_eq!(pixels(run(case, false)), pixels(run(case, true)));
    }
    let mut c = Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_millis(300))
        .measurement_time(Duration::from_secs(1))
        .without_plots()
        .configure_from_args();
    let mut group = c.benchmark_group("stroke_union");
    group.sampling_mode(SamplingMode::Flat);
    for case in &cases {
        for checked in [false, true] {
            group.bench_function(
                BenchmarkId::new(case.name, if checked { "checked" } else { "legacy" }),
                |b| b.iter(|| drop(black_box(run(case, checked)))),
            );
        }
    }
    group.finish();
    c.final_summary();
}
