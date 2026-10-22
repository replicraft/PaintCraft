# Whole-batch preparation: correctness adoption with measured regressions

Complete built-in batch preparation is selected for its decoded-admission and
logical-source guarantees. These measurements do **not** establish a speedup.
Diagonal pencil and mixed flood/duplicate workloads regress substantially. The
next performance work must reduce retained geometry and redundant preparation
without weakening whole-batch validation, cancellation or atomic publication.

## Artifacts and method

Measured source: `4fbabe33e603ebbc8cf63d4156272623a0c14c9f`.
Production validation: `f283398`; the subsequent measured-source change only
strengthens the benchmark driver and its independent CLI semantic verifier.
The later Clear Canvas icon commit is outside this measured source.

Parent CPU-only CLI source is `ff50020`; its full source hashes are retained in
[baseline-cli-manifest.json](baseline-cli-manifest.json). The real-library driver
uses source-pinned sequential executor support and the current raster primitives
for both modes. Thus library comparisons isolate editor preparation changes,
while the two frozen CLI artifacts include the complete parent/current application
changes, recorded history and canonical file IO. Frozen binary hashes, complete
source/fixture identities, toolchain, platform and matched CPU-only feature graph
are in [manifest.json](manifest.json). No GPU is linked into either timed CLI.

Seven cases use 100 operations, except mixed lineage with 42 operations. The
1000-square white-source stroke fixtures retain earlier workload geometry. Mixed
lineage selects a local region and repeats wall painting, duplication, flood fill,
additional child painting and deletion eight times, then clears selection. Dense
imports one million reproducible opaque pixels; its raw RGBA, PNG and independently
reconstructed PCND inputs are retained. Default control performs 100 white fills.
The dense case uses the horizontal pencil geometry over its populated source.

Criterion retains 20 samples for each case/mode: **280 statistical samples**.
Input acquisition is outside these loops; execution and result destruction are
inside. Hyperfine retains three forward/reverse/forward sessions, each with
20 measured runs and three warmups: **1,680 workload observations** plus **180
startup calibrations**, or **1,860 original process observations**. Library process
runs include acquisition and repeat the edit five times, except default control
with 100 repetitions. CLI processes execute the actual apply/history/save path.
Library and CLI durations therefore have different workloads and must not be
compared as absolute cross-tier costs. One library executable has one calibration
per session; each distinct CLI artifact has its own calibration.

This is one uncontrolled Apple Silicon macOS host without affinity, thermal or
power controls. No intentional concurrent builds/tests/timing were run. Startup
is reported without subtraction. Short process controls, long tails, session
drift, opposing tier conclusions and RSS remain in the original exports.

## Exact correctness and resource checks

All four before/after checkpoints match. Both library modes have identical full
document semantics, selection, aliases and decoded peaks. Each library semantic
SHA is independently checked against the actual CLI output's reconstructed raw
PCND pixels and ordered document metadata. CLI archives and every ZIP member
are byte-identical between parent/current, including publication identity. The
input dense image also matches its independently decoded PCND pixels. Source,
frozen executable and fixture hashes were checked after measurement; this audit
rechecked the source through `git show` of the measured commit.

[correctness.json](correctness.json) retains checkpoint identities;
[audit.json](audit.json) records exact counts and scope. Raw logs, fixtures and
canonical outputs use lossless deterministic gzip. [compressed-artifacts.json](compressed-artifacts.json)
records original/archive hashes and lengths. Hyperfine's original schema-v2 JSON
exports are independently retained in `.json.gz`; the compact `.json` values are
identical, with both byte hashes in the corresponding `.identity.json`.
Criterion retains raw samples, estimates and benchmark identities.

## Statistical execution means

Values are milliseconds per library edit; brackets are 95% mean confidence
intervals from Criterion. The eraser intervals overlap. Other comparisons retain
their raw distributions rather than promoting nonoverlap to a universal result.

| Case | Sequential mean [CI] | Planned mean [CI] | Planned / sequential |
| --- | ---: | ---: | ---: |
| horizontal_pencil | 1.3654 [1.3589, 1.3726] | 1.4652 [1.4556, 1.4789] | 1.07x |
| diagonal_pencil | 6.9151 [6.7681, 7.0815] | 10.8166 [10.7711, 10.8628] | 1.56x |
| smooth_brush | 12.4046 [12.3184, 12.5310] | 13.0146 [12.9769, 13.0540] | 1.05x |
| smooth_eraser | 3.4659 [3.1801, 3.8778] | 3.7478 [3.7240, 3.7795] | 1.08x |
| mixed_lineage | 0.8775 [0.8570, 0.9021] | 2.9683 [2.9169, 3.0367] | 3.38x |
| default_control | 0.0308 [0.0278, 0.0355] | 0.0239 [0.0238, 0.0239] | 0.78x |
| dense | 1.3720 [1.3658, 1.3791] | 1.4607 [1.4575, 1.4640] | 1.06x |

## Process means by session

Each cell is sequential / planned milliseconds, followed by planned wall-time
change. Negative means faster in that session. No session or slow control is
excluded. Full median/min/max, CPU and RSS summaries are retained in
[analysis-cli.json](analysis-cli.json) and [analysis-library.json](analysis-library.json).

### CPU-only CLI

| Case | Session 1 | Session 2 | Session 3 |
| --- | ---: | ---: | ---: |
| horizontal_pencil | 23.144 / 23.506 (+1.6%) | 23.621 / 23.497 (-0.5%) | 24.079 / 21.224 (-11.9%) |
| diagonal_pencil | 28.688 / 33.575 (+17.0%) | 29.816 / 42.899 (+43.9%) | 26.123 / 31.651 (+21.2%) |
| smooth_brush | 34.008 / 35.315 (+3.8%) | 34.771 / 35.390 (+1.8%) | 33.075 / 32.718 (-1.1%) |
| smooth_eraser | 25.672 / 25.269 (-1.6%) | 27.064 / 27.490 (+1.6%) | 23.262 / 24.166 (+3.9%) |
| mixed_lineage | 16.188 / 18.407 (+13.7%) | 16.585 / 18.198 (+9.7%) | 15.112 / 17.879 (+18.3%) |
| default_control | 12.864 / 12.967 (+0.8%) | 13.382 / 12.908 (-3.5%) | 13.061 / 13.364 (+2.3%) |
| dense | 48.086 / 50.055 (+4.1%) | 46.806 / 48.647 (+3.9%) | 48.397 / 42.657 (-11.9%) |

### Real-library process

| Case | Session 1 | Session 2 | Session 3 |
| --- | ---: | ---: | ---: |
| horizontal_pencil | 9.294 / 11.121 (+19.7%) | 9.197 / 10.349 (+12.5%) | 9.256 / 9.623 (+4.0%) |
| diagonal_pencil | 36.996 / 57.779 (+56.2%) | 35.990 / 60.500 (+68.1%) | 35.473 / 57.453 (+62.0%) |
| smooth_brush | 66.046 / 68.591 (+3.9%) | 65.558 / 68.537 (+4.5%) | 65.080 / 68.687 (+5.5%) |
| smooth_eraser | 18.689 / 21.350 (+14.2%) | 18.539 / 21.783 (+17.5%) | 18.055 / 21.699 (+20.2%) |
| mixed_lineage | 6.053 / 16.679 (+175.5%) | 6.571 / 16.731 (+154.6%) | 6.296 / 20.610 (+227.3%) |
| default_control | 4.725 / 4.398 (-6.9%) | 4.859 / 4.874 (+0.3%) | 6.184 / 4.522 (-26.9%) |
| dense | 10.669 / 11.616 (+8.9%) | 10.824 / 11.648 (+7.6%) | 10.931 / 11.542 (+5.6%) |

CPU CLI startup sequential/planned means are 1.294/1.379, 1.612/1.566 and
1.265/1.242 ms. Shared library startup means are 1.462, 1.485 and 1.398 ms.
Default/no-op and horizontal/dense CLI controls vary with session noise; the
library diagonal and mixed regressions repeat in every session. Smooth eraser
has overlapping statistical intervals but a 14.2–20.2% slower library process.
Smooth brush retains 3.9–5.5% library process overhead. Dense retains 5.6–8.9%
library process overhead despite its final CLI session improvement.

## Memory and CPU controls

| Case | Equal decoded peak bytes | Sequential metadata peak | Planned metadata peak |
| --- | ---: | ---: | ---: |
| horizontal pencil | 1,048,576 | 568 | 23,568 |
| diagonal pencil | 1,048,576 | 49,432 | 4,111,632 |
| smooth brush | 1,048,576 | 4,888 | 265,488 |
| smooth eraser | 1,048,576 | 4,888 | 265,488 |
| mixed lineage | 786,432 | 3,192 | 36,728 |
| default control | 0 | 160 | 1,840 |
| dense | 1,048,576 | 568 | 24,072 |

These are declared account peaks, not physical ceilings. Retaining complete
compiled descriptor sequences increases the diagonal metadata peak substantially.
Diagonal library median RSS rises from 5.77–5.86 MiB to 20.31–21.28 MiB;
CLI median RSS rises from 5.63–5.86 MiB to 10.02–10.20 MiB. Process repetition,
allocator reuse and acquisition affect these measurements. They do not imply
that declared metadata equals RSS or that the difference is fully explained.

Diagonal library CPU means rise from 34.52–35.01 ms to 56.16–56.94 ms. Mixed
library CPU means rise from 5.40–5.70 ms to 15.76–16.96 ms; mixed CLI CPU means
rise from 4.72–4.98 ms to 6.72–7.10 ms. This is more than a startup-only wall
artifact. Mixed median library RSS also increases, from 3.01–3.16 MiB to
3.54–4.45 MiB. Other RSS controls remain mixed and no general reduction is claimed.

## Decision and remaining work

Keep complete built-in validation and decoded-peak admission as the correctness
baseline. Do not advertise it as a performance optimization. Investigate retained
repeated plans, duplicate indexing and flood/branch preparation using these exact
fixtures and a new independently audited comparison. Preserve this regression
record when an improved implementation is measured.

[Validation](../../../docs/validation/batch-preparation-2026-10-22/validation.json)
retains 908 passing workspace tests (26 explicit ignores, 83 suites), strict
all-target Clippy, 122 optimized raster tests, 118 optimized editor tests,
54 required-Metal renderer tests, 236 required-GPU desktop tests (eight ignores),
release artifacts and default/single-layer isolated 30-frame startup checks.
Earlier failed development checks and the pre-read-elision validation remain
separate from the final production checks. The tooling-only semantic verifier
follow-up reran Clippy and benchmark compilation, not the production suite.

This completes preparation for the current built-in command grammar only.
Aggregate metadata/geometry/font/flood/history/GPU resource reservation, global
physical accounting, branching-history policy, fairness, native sustained-input
and cancellation/display latency, future command preparation and the broader
feature roadmap remain open. No dependency, wire/file policy or rendering-default
change follows from these measurements.
