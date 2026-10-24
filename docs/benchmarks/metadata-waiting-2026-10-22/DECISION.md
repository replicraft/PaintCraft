# Scoped metadata demand: correctness adoption with measured overhead

Keep scoped live-plus-pending metadata demand and separate native metadata waiting.
These preserve retry readiness and edit/file ordering under shared pressure. The
comparison is mixed, with a small projected-tiny regression and noisy process
controls. It does not establish a general throughput improvement.

## Sources, fixtures and required output

Baseline source is `858753c`; scoped candidate source is `1b83c2d`. Frozen baseline
CLI/library binaries are the previous shared-ownership comparison's charged
artifacts. Their manifests link that evidence and retain full source hashes. Both
CLIs have identical CPU-only dependency feature trees without wgpu. Candidate
source, lockfile, toolchain, fixtures and all four binaries are hashed. No task
builds or tests intentionally overlap timing. The macOS host is uncontrolled:
no affinity, power or thermal controls.

The unchanged runner labels the candidate `charged`; both versions already have
shared metadata ownership. This comparison adds attempt scopes to the normal
paint-preparation path. CLI cases apply 100 operations to the same 1000-square
PCND, retain recorded history and atomically write separate canonical outputs.
Library cases compare real materialized, chained-projection and combined-program
preparation/flood primitives. The benchmark driver and fixtures are source-identical.
Native waiting and wake routing are tested separately; these CPU processes do not
measure the GUI scheduler.

All preflight and three postflight checkpoints pass: CLI archives and every ZIP
member are byte-identical, and library every-pixel/region/resource logs match.
After timing, all source/binary identities were checked again, including baseline
source hashes against committed history. Compressed artifacts preserve exact
uncompressed identities. No failed build or partial export is timed as a result.

## Statistical measurements

Each artifact retains 20 Criterion samples for each of 12 case/mode combinations:
480 raw statistical samples. Values below are mean milliseconds per complete
preparation/classification/destruction attempt with 95% bootstrap intervals.

| Case/mode | Baseline mean [interval] | Scoped mean [interval] |
|---|---:|---:|
| tiny materialized | 0.01238 [0.01214, 0.01265] | 0.01210 [0.01183, 0.01238] |
| tiny projected | 0.00513 [0.00511, 0.00516] | 0.00549 [0.00545, 0.00553] |
| tiny combined | 0.00851 [0.00601, 0.01161] | 0.00545 [0.00542, 0.00547] |
| wall materialized | 1.85373 [1.84256, 1.86530] | 1.83701 [1.82794, 1.84626] |
| wall projected | 1.73272 [1.72441, 1.74179] | 1.79495 [1.73985, 1.87271] |
| wall combined | 1.78575 [1.76529, 1.80904] | 1.76453 [1.75147, 1.77867] |
| overlap materialized | 0.96112 [0.94642, 0.97915] | 0.94987 [0.94304, 0.95890] |
| overlap projected | 9.20736 [9.13264, 9.29549] | 9.09778 [9.06726, 9.13125] |
| overlap combined | 0.53182 [0.52078, 0.55204] | 0.53022 [0.52941, 0.53100] |
| default-control materialized | 0.51529 [0.49831, 0.53991] | 0.49871 [0.49666, 0.50100] |
| default-control projected | 2.48047 [2.32865, 2.71015] | 2.47021 [2.32096, 2.69718] |
| default-control combined | 2.32133 [2.28023, 2.36012] | 2.31739 [2.27316, 2.37364] |

Projected tiny increases about 7% with disjoint intervals. Combined tiny's baseline
interval is wide and overlaps the candidate; its lower candidate mean is not
strong evidence of an improvement. Most larger cases overlap or have small mixed
differences. These samples compare account/scope overhead within each mode, not
a new production choice among materialized, projected and combined execution.

## Process measurements and retained controls

Three forward/reverse/forward Hyperfine sessions retain 20 observations and three
warmups per artifact/case. There are 960 workload observations and 240 startup
calibration observations, 1,200 total, all exit zero. Library process cases use
combined programs with 10,000/100/100/100 repetitions for tiny/wall/overlap/default
controls. No calibration subtraction or outlier removal. Positive percentage means
the candidate took longer. Original schema-v2 JSON is retained compressed unchanged;
compact JSON has identical parsed values.

| Process case | Baseline/scoped means ms, sessions 1–3 | Time changes, sessions 1–3 |
|---|---|---|
| cli horizontal_pencil | 21.33/20.57, 20.83/21.74, 22.45/22.17 | -3.6%, +4.4%, -1.3% |
| cli diagonal_pencil | 29.37/30.56, 29.62/29.06, 30.55/31.04 | +4.1%, -1.9%, +1.6% |
| cli smooth_brush | 32.76/32.77, 31.76/32.75, 35.15/33.21 | +0.0%, +3.1%, -5.5% |
| cli smooth_eraser | 39.33/23.91, 23.06/23.36, 23.62/24.07 | -39.2%, +1.3%, +1.9% |
| cli calibration | 1.20/1.32, 1.40/1.46, 1.43/1.46 | +9.9%, +4.0%, +2.0% |
| library tiny | 54.19/57.31, 77.30/63.83, 63.89/58.03 | +5.8%, -17.4%, -9.2% |
| library wall | 181.20/177.26, 175.16/178.26, 177.41/181.26 | -2.2%, +1.8%, +2.2% |
| library overlap | 55.08/57.38, 55.90/56.78, 56.12/56.52 | +4.2%, +1.6%, +0.7% |
| library default-control | 243.13/230.79, 238.36/229.99, 242.94/229.66 | -5.1%, -3.5%, -5.5% |
| library calibration | 1.37/1.34, 1.41/1.46, 1.48/1.42 | -2.1%, +3.9%, -3.7% |

CLI horizontal/diagonal/brush results change direction across sessions. Eraser
session 1 has a slow baseline: median 30.13 ms and maximum 120.80 ms versus candidate
23.59/28.37 ms. Its apparent 39% mean improvement does not repeat in sessions 2/3,
where the candidate takes 1.3/1.9% longer. The whole original session stays retained;
the cause is unproven.

Library tiny candidate medians are slower in every session (56.84/58.56/57.36 ms
versus 53.84/54.95/54.67 ms). Baseline long observations reach 356.46/224.08 ms in
sessions 2/3, making the mean ratios misleading in isolation. Candidate session 2
also retains a 128.21 ms observation. Overlap process overhead is 0.7–4.2%; wall
changes direction. The default/no-op control has shorter candidate process means
while its statistical intervals overlap. Host variation and tier disagreement
prevent attributing every observed change to scopes. No extra preferred rerun
replaces these original observations.

Per-session wall medians, min/max, mean CPU time and median peak RSS are retained in
`analysis-process.json`; full original metrics remain in raw exports. CLI workload
median RSS ranges about 8.0–9.4 MiB, library about 1.5–1.8 MiB, with small changes
in both directions. These short processes do not establish a physical-memory
ceiling or reclamation guarantee. Payload counters exclude allocator/Arc/scope
headers and mutex controls.

## Adoption and remaining scope

Scoped accounts now price known live-plus-pending demand and release independently.
Native feasible metadata failures retain owner/base, FIFO, preview and file barriers;
separate watches cannot turn metadata demand into decoded-history reclamation.
Escape, stale bases, hard limits, original realms and retry/no-spin behavior have
source-pinned regressions.

This is best-effort admission on retry, not atomic metadata reservation. Future
preparation can discover a larger requirement. Whole-batch/source-lineage planning,
aggregate geometry/font/flood/service budgets, cross-window fairness, physical
accounting and native input/display/cancellation tails remain open. Keep existing
sequential editor execution; these measurements do not qualify combined-program
integration or change rendering defaults.

See [the contract](../../../docs/METADATA_WAITING.md) and
[source-pinned validation](../../../docs/validation/metadata-waiting-2026-10-22/validation.json).
