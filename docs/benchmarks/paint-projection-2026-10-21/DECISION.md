# Logical paint preparation: useful primitive, no chained-view default

Keep ordinary sequential editor replay. The source-independent read projection is
a correctness/capability baseline for whole-batch planning, not a globally selected
acceleration. Tiny preparation benefits coexist with a wall tie and severe
many-plan/no-op regressions. A retained program needs bounded aggregate ownership,
efficient combined lookup and uniform/no-op proofs before editor integration.

## Identity and required work

Measured source is `1863c6c`, production primitive source `d4fd5a3`.
`manifest.json` pins every source, toolchain, one frozen optimized executable and
fixture definitions. Both variants call real library paint/flood paths in that
same binary. Inputs are authored in the pinned Rust driver, not a timing model.
No task builds/tests intentionally overlapped timing. This is one uncontrolled
macOS host without affinity, power or thermal controls.

Each iteration builds paint descriptors and either materializes private raster
paint before flood, or retains read projections and classifies the same exact
logical pixels. Construction, classification, cleanup and output destruction are
inside timing. Both require the same complete flood region. Before Criterion and
after every process session, all prior-paint pixels and canonical region spans
match exactly. Correctness/resource outputs remain byte-identical across checks.
The existing editor is not using chained projections for whole-batch execution.

Tiny is 8x8 with one wall plan; wall is 512x64 with one wall plan; overlap is 128x64
with 64 source-over rectangle plans; default-control is a 512x64 replacement with
the existing implicit hidden-RGB default. Per-process repetition counts are 10,000,
100, 100 and 100 respectively. Repetitions rebuild real inputs/private attempts;
they do not time a million cached result hits. Process time includes setup and
teardown and is not a per-pointer-event sample.

## Statistical and process evidence

Criterion retains 20 raw samples for both variants of all four cases (160 total).
Means and bootstrap intervals below are milliseconds per complete iteration.

| Case | Materialized mean [interval] | Projected mean [interval] |
|---|---:|---:|
| tiny | 0.01211 [0.01195, 0.01228] | 0.00476 [0.00469, 0.00487] |
| wall | 1.82797 [1.80317, 1.86858] | 1.77251 [1.76711, 1.77819] |
| overlap | 0.94868 [0.93584, 0.96876] | 9.56037 [9.50869, 9.61571] |
| default-control | 0.50205 [0.50046, 0.50373] | 2.22485 [2.19488, 2.25264] |

Hyperfine retains 20 runs and three warmups in three forward/reverse/forward
sessions: 480 workload observations plus 60 same-executable calibration observations,
all exit zero. Original schema-v2 exports are compressed unchanged; compact JSON
has identical parsed values. No outlier removal or calibration subtraction.
Ratios above one favor projection; values below one favor materialization.

| Case | Materialized/projected process means ms, sessions 1–3 | Ratio range |
|---|---|---:|
| tiny | 125.35/48.89, 123.76/49.60, 124.59/49.74 | 2.495–2.564x |
| wall | 185.79/184.48, 184.34/182.23, 185.20/190.25 | 0.973–1.012x |
| overlap | 95.92/964.29, 102.23/969.56, 96.97/955.97 | 0.099–0.105x |
| default-control | 53.24/232.81, 52.82/231.05, 52.31/232.09 | 0.225–0.229x |

Tiny gains are consistent across tiers. Wall has mixed/tied process results despite
a small statistical difference. Sixty-four chained projections perform many source
lookups for each examined pixel; the 9–10x regression makes this composition style
unsuitable as a default batch implementation. Source-independent no-op descriptors
cannot exploit the raster's existing default proof and regress by over 4x. Keep
these controls when comparing a combined program or context-specific proofs.

## Resource evidence and limits

The materialized tiny/wall/overlap variants each reach 262,144 declared decoded
bytes; default-control reaches zero. Projection has no decoded lease and retains
312, 312, 19,968 and 344 declared projection payload bytes respectively. Those
payload counts include descriptors and usable map/vector capacities only, excluding
allocator/hash controls, source chains, temporary plan vectors, flood scratch and
other services. Materialized projection-payload zero does **not** mean its temporary
paint index allocates nothing. These are distinct declarations, not RSS estimates.

`analysis.json` and `summary.json` preserve each session's measured median process
RSS. Tiny/wall/overlap use less RSS locally; the no-op control is mixed/slightly
higher. No physical ceiling or universal memory-reclamation claim follows from
logical counters or one host's short runs. Input/display/cancellation tails, CPU/GPU
crossover and full editor/batch/history/file costs are not qualified here.

## Next implementation

Retain the exact read primitive and its quotas/cancellation/source-failure checks.
Build a bounded complete logical program with per-tile lookup rather than repeated
independent source chains, preserve layer lineage and exact rounding, account all
retained plans/scratch, then plan decoded demand and reserve before replay. Measure
that integrated path with the same slow controls and independent required pixels.
This comparison establishes costs of current preparation primitives, not completion
of whole-batch admission or a roadmap gate.

See [the contract](../../../docs/PAINT_PROJECTION.md) and
[source-pinned validation](../../../docs/validation/paint-projection-2026-10-21/validation.json).
