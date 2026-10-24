# Source-bound paint lineage: preserve context, retain mixed timing

Keep the source-bound preparation primitive for immutable branch identity and
contextual no-op proofs. It avoids the arbitrary-source program's default/no-op
penalty in this fixture, but adds tiny/wall overhead and does not outperform every
materialized control. The editor remains sequential; no full-batch or rendering
default is changed by this experiment.

## Source, workload and exact required output

Measured source is `bd97b1b`, including the concurrent-release teardown fix. One
frozen optimized paint_lineage driver compares materialized, combined arbitrary-
source and bound-source modes with the same dependencies and fixture code. The
manifest pins the executable, source files, lockfile and toolchain. No task builds,
tests or startup checks overlap timing. This is an uncontrolled local macOS host
without affinity/power/thermal controls.

The four fixtures retain 8x8 tiny paint, a 512x64 wall, 64 overlapping plans on
128x64, and a 512x64 implicit-default/no-op control. Each iteration includes plan
construction, exact flood classification, output destruction and all source/view
setup/destruction. Bound mode includes capturing an owned sparse raster, an append
program and its reader. Process repetition counts are 10,000/100/100/100.

All three modes match every required prior-paint pixel and canonical flood region
before timing; all four correctness checkpoints retain identical resource/output
logs. Source/binary hashes match after timing. These flat preparation cases do not
measure branching-chain throughput, editor structural integration, file IO, native
input/display/cancellation latency or physical memory ceilings.

## Statistical measurements

There are 20 raw Criterion samples per case/mode: 240 samples. Mean milliseconds
per complete preparation/classification/destruction attempt and 95% bootstrap
intervals follow. No slow sample or baseline is deleted.

| Case | Materialized mean [interval] | Combined mean [interval] | Bound mean [interval] |
|---|---:|---:|---:|
| tiny | 0.01246 [0.01233, 0.01258] | 0.00689 [0.00542, 0.00977] | 0.00620 [0.00618, 0.00623] |
| wall | 1.82760 [1.81465, 1.84197] | 1.74552 [1.73983, 1.75308] | 2.01963 [2.01223, 2.02782] |
| overlap | 0.94925 [0.94384, 0.95575] | 0.55696 [0.53652, 0.58754] | 0.58148 [0.50972, 0.66741] |
| default-control | 0.51113 [0.50114, 0.52748] | 2.28513 [2.25265, 2.31675] | 0.57747 [0.57490, 0.58075] |

Bound wall is about 15.7% slower than combined, with disjoint intervals. Bound
default is about 3.96x faster than combined but about 13% slower than materialized.
Tiny combined has a wide interval; it does not establish that bound is faster.
Overlap bound/combined intervals overlap despite their point-estimate difference.

## Process observations, controls and resources

Three forward/reverse/forward Hyperfine sessions retain 20 observations and three
warmups for each mode/case. There are 720 workload observations plus 60 startup
calibration observations (780 total), all exit zero. Calibration uses the same
executable once per session and is not subtracted. Original schema-v2 JSON is
compressed unchanged; compact JSON has identical parsed values and both hashes.

| Case | Materialized/combined/bound mean ms, sessions 1–3 | Bound time change vs combined, sessions 1–3 |
|---|---|---|
| tiny | 129.53/59.01/65.77, 132.34/56.83/65.90, 126.77/60.39/70.23 | +11.5%, +15.9%, +16.3% |
| wall | 209.30/177.21/207.91, 188.73/186.06/222.23, 203.36/216.04/211.80 | +17.3%, +19.4%, -2.0% |
| overlap | 99.38/56.96/54.94, 96.60/55.94/52.85, 99.72/56.97/54.81 | -3.6%, -5.5%, -3.8% |
| default-control | 53.68/232.53/61.32, 52.88/234.46/60.02, 52.73/236.59/63.84 | -73.6%, -74.4%, -73.0% |

Tiny bound takes 11.5–16.3% longer than combined, while reducing materialized
process time by 44.6–50.2%. Wall bound is 17.3/19.4% slower than combined in the
first two sessions; the third mean ratio reverses amid a 371.64 ms combined tail.
Combined wall medians are 176.67/180.76/194.59 ms versus bound 203.91/218.25/209.72 ms.
Materialized wall session 1 retains a 553.01 ms observation. All original tails,
medians and CPU times remain; a preferred rerun does not replace them.

Overlap bound process means are 3.6–5.5% shorter than combined, but statistical
intervals overlap. Default/no-op bound is 3.71–3.91x faster than combined across
process sessions, while taking 13.5–21.1% longer than materialized. This addresses
a known arbitrary-source no-op cost without establishing a universal winner.
Calibration means are 1.540/1.600/1.662 ms; raw observations remain independent of
workload timings. Host drift and tier disagreement limit causal/dispatch claims.

| Case | Materialized decoded peak bytes | Combined declared payload bytes | Bound declared payload bytes |
|---|---:|---:|---:|
| tiny | 262144 | 312 | 584 |
| wall | 262144 | 312 | 584 |
| overlap | 262144 | 3192 | 3464 |
| default-control | 0 | 344 | 240 |

Bound payload includes the source node and reader-vector capacity; combined has
no equivalent captured-source owner. Shared ancestors must not be counted twice
when totaling real branches. These are declared payloads, excluding allocator/Arc/
hash controls and flood scratch, not interchangeable physical-memory totals. Bound
retains original decoded leases when the supplied source has pixels; these fixtures
start from sparse uniform rasters. No decoded copy is made by logical preparation.

Process workload median peak RSS ranges roughly 1.5–4.0 MiB across modes. Bound and
combined are around 1.5–1.8 MiB here; materialized painted fixtures are around
2.7–4.0 MiB. Tiny/default controls vary, and process RSS is not a physical admission
or reclamation proof. analysis-process.json retains per-session mean/median/min/max
wall time, mean CPU time and median RSS; original exports retain every metric.

## Adoption scope

The captured-source API owns immutable input leases, supports freeze/fork/expansion,
and never exposes an unrelated-source callback or detached filtered plan. Missing/
uniform-default proofs therefore stay bound to the inputs that justify them.
Read and teardown walks are iterative, including concurrent ancestor release;
source/program/reader metadata uses shared or explicitly reserved ownership.
Independent pixels/BFS, cancellation, quota, source-COW and deep small-stack
regressions are validated separately from timing.

Keep this capability as a prerequisite for complete batch planning. Structural
editor integration, contextual tile-ownership simulation, complete decoded peak
and cumulative plans, joint resource admission, aggregate geometry/font/flood
budgets, scheduling fairness and native latency remain open. The editor continues
to execute sequentially; neither flat benchmark gains nor model tests complete
the whole-batch/native gates.

See [the contract](../../../docs/PAINT_LINEAGE.md) and
[source-pinned validation](../../../docs/validation/paint-lineage-2026-10-22/validation.json).
