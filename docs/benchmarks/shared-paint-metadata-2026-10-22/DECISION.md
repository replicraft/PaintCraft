# Shared paint metadata: ownership safety with measured overhead

Keep shared descriptor/index admission and copy-on-write storage ownership. These
are correctness requirements, not a selected throughput optimization. The matched
comparison retains tiny preparation overhead, a large original eraser slowdown,
follow-up variability and mixed controls. Do not infer a general speedup, physical
memory reduction or optimal 256 MiB allowance from this experiment.

## Source, artifacts and required output

Charged source is `858753c`. CPU-only CLI baseline is `e2aab09`; the frozen library
baseline is `207227b` (production `c00140c`). Separate baseline manifests retain
complete source identities. Both CLIs use byte-identical dependency feature trees
without wgpu, the same toolchain/dependency lock and the same release build command.
The library driver/fixtures and required outputs are unchanged across artifacts.
Every executable/source/input is hashed; no task builds/tests intentionally overlap
timing. This is an uncontrolled macOS host without affinity/power/thermal controls.

CLI cases read and hash-check the same 1000-square PCND, apply 100 atomic operations,
publish recorded history, encode canonical PCND and atomically replace separate
outputs. The four cases retain horizontal/diagonal pencil, smooth brush and smooth
eraser. Exact archive and every ZIP member match before and after all sessions.
Library preflights compare every prior-paint pixel and canonical flood region for
materialized/chained/combined modes. Their resource-output logs remain identical
across old/new executables and every pre/postflight.

## Raw measurements

Both frozen library executables retain 20 Criterion samples for each of 12
case/mode combinations: 480 statistical samples. Means and bootstrap intervals
below are milliseconds per complete preparation/classification/destruction attempt.

| Library case/mode | Baseline mean [interval] | Charged mean [interval] |
|---|---:|---:|
| tiny materialized | 0.01222 [0.01209, 0.01236] | 0.01236 [0.01228, 0.01245] |
| tiny projected | 0.00511 [0.00472, 0.00573] | 0.00512 [0.00510, 0.00515] |
| tiny combined | 0.00488 [0.00474, 0.00512] | 0.00512 [0.00510, 0.00515] |
| wall materialized | 2.01160 [1.81258, 2.33699] | 1.81168 [1.80463, 1.81843] |
| wall projected | 1.79983 [1.79547, 1.80421] | 1.83436 [1.71538, 1.99345] |
| wall combined | 1.79887 [1.79218, 1.80520] | 1.72049 [1.71325, 1.72821] |
| overlap materialized | 0.93110 [0.92839, 0.93401] | 0.93702 [0.93427, 0.93988] |
| overlap projected | 10.24819 [9.58956, 11.33427] | 9.07768 [9.04798, 9.11185] |
| overlap combined | 0.60707 [0.60456, 0.60999] | 0.52833 [0.52698, 0.52989] |
| default-control materialized | 0.50503 [0.50184, 0.50871] | 0.50456 [0.50214, 0.50786] |
| default-control projected | 2.27138 [2.24194, 2.29836] | 2.39322 [2.27466, 2.58063] |
| default-control combined | 2.31179 [2.22080, 2.44567] | 2.34989 [2.31941, 2.37971] |

The initial Hyperfine run retains three forward/reverse/forward sessions, 20 runs
and three warmups: 960 workload observations plus 240 calibration observations
(1,200 total), all exit zero. Library process cases exercise combined programs;
all three modes retain statistical evidence above. CPU CLI is the current real
application consumer of ordinary paint accounting. Library process repetition
counts remain 10,000/100/100/100. No calibration subtraction or outlier deletion.
Positive time change is overhead, negative change is a shorter observed run.

| Process case | Baseline/charged means ms, sessions 1–3 | Charged time-change range |
|---|---|---:|
| cli horizontal_pencil | 22.64/22.00, 21.97/22.05, 21.02/21.22 | -2.8% to +0.9% |
| cli diagonal_pencil | 29.76/28.66, 32.27/30.46, 30.51/29.26 | -5.6% to -3.7% |
| cli smooth_brush | 32.37/32.54, 32.72/32.66, 31.76/32.22 | -0.2% to +1.4% |
| cli smooth_eraser | 23.44/23.67, 29.71/44.24, 22.92/23.45 | +1.0% to +48.9% |
| library tiny | 49.87/54.05, 50.01/54.07, 48.77/59.27 | +8.1% to +21.5% |
| library wall | 183.80/185.70, 183.71/176.37, 188.32/173.47 | -7.9% to +1.0% |
| library overlap | 67.22/55.01, 62.79/55.24, 61.39/54.41 | -18.2% to -11.4% |
| library default-control | 257.29/230.13, 232.35/233.71, 227.11/231.70 | -10.6% to +2.0% |

Horizontal pencil and smooth brush are near tied/mixed across sessions. Diagonal
pencil and combined overlap have shorter observed charged times, while combined
tiny costs 8–22% more in process measurements. Statistical/process disagreement
and host variation prevent attributing every difference to accounting. No general
acceleration claim follows. Per-session RSS, CPU time, medians and full ranges are
retained in `analysis-process.json`; raw metrics include instructions where available.

## Preserved eraser slowdown and focused follow-up

Original session 2 eraser is 29.71/44.24 ms baseline/charged (48.9% mean overhead),
with medians 24.79/38.83 ms and a charged maximum of 115.28 ms. This is a broad
interval slowdown, not just one tail sample. Other original eraser sessions show
about 1–2% mean overhead. Calibration alone cannot identify the cause.

A separately labeled follow-up repeats only this case and calibration with the
same frozen binaries/fixtures in reverse/forward/reverse order: another 120 workload
and 120 calibration observations, all exit zero, with exact output checks after
each session. Follow-up mean changes are +4.75%, -3.87% and -0.32%; their full
medians/tails remain, including an 88.16 ms baseline sample. The original large
slowdown did not repeat in these sessions, but remains part of the evidence. Its
cause is unproven. Never replace the original result with a preferred follow-up.
The archived execution script and parent/script hashes make this extra scope
reviewable separately from the original runner.

Across both scopes there are 1,440 process observations (1,080 workloads and 360
calibration), plus 480 statistical samples. Original schema-v2 exports are compressed
unchanged; compact JSON preserves identical parsed values. All source/binary and
canonical output postflights pass. This does not measure native event p99.

## Adoption and remaining scope

The shared realm now owns declared descriptor/index capacities across programs
and callers. Clone shares one immutable allocation/charge; mutation copies under
admission, preserving exact capacity. Storage frees before charge-release callbacks.
Fixed per-plan denials stay fixed; shared denials identify PaintMetadata and never
enter decoded-history pressure retry. Atomic editor rollback under interference
and safe cancellation/source snapshots are separately validated.

Payload policy excludes allocator/hash controls, Arc headers, transient reallocations,
geometry/font/flood buffers and other services. Measured short-process RSS is not
a physical ceiling or a reclamation proof. Complete-attempt metadata waiting,
lineage/whole-batch planning, aggregate scratch and native latency/interference
qualification remain. Preserve all slow controls in future integrated comparisons.

See [shared ownership](../../../docs/SHARED_PAINT_METADATA.md) and
[source-pinned validation](../../../docs/validation/shared-paint-metadata-2026-10-22/validation.json).
