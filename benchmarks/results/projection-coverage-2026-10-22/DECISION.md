# Projection coverage: exact source reads with bounded write envelopes

Selected change: reject points outside a conservative clipped write envelope before
looking up a program's tile index. Each in-bounds read still queries its preceding
source once and propagates its errors. Outside-layer transparency and cancellation
remain unchanged. Construction and appends grow the envelope with checked bounds;
source-bound omission may make the envelope conservative, never too small.

Production source is `e6e8896`; measured tool/source snapshot is `1c9afaf`.
Before artifacts are the source-pinned descriptor-sharing implementation at
`40be7e6`, supplied through `baseline-artifacts.json`. The older batch manifest
owns reproducible fixture bytes, not the actual before executable selection.
`manifest.json` records the actual four binary identities, full source/fixture
hashes, matched CPU-only feature graph, tool versions and platform. The library
driver is unchanged. All four before/after checkpoints match exact library
semantics/decoded peaks and canonical CLI archives. Independent raw PCND pixel
reconstruction matches each library semantic hash.

480 statistical samples and 3,120 original schema-v2 process observations are
retained: 2,880 workloads and 240 calibrations over three forward/reverse/forward
sessions, with 20 runs and three warmups. Seven original repeated cases and five
changing-color controls retain their workload scopes from the earlier experiment.
Library statistics exclude acquisition and include result destruction; processes
include acquisition and five edits (100 for the default control). CLI measurements
include apply, history and file IO. Absolute tiers therefore differ.

One uncontrolled Apple Silicon macOS host; no affinity, thermal or power controls.
No intentional concurrent task build/test/timing ran. No sample is excluded or
startup-subtracted. Original Hyperfine exports, Criterion samples, medians/tails,
CPU, RSS, cycles and instructions remain alongside all controls in `analysis-*.json`.
Lossless raw input/output/log archives have original/packed hashes and lengths in
`compressed-artifacts.json`; compact Hyperfine/correctness JSON retains original
bytes and value-equivalence identities separately. `audit.json` pins exact counts.

## Target result

The mixed flood/duplicate library mean changes from **2.9433 ms** [2.9178, 2.9731]
to **1.1110 ms** [1.0932, 1.1348], a **2.65x** improvement in this statistical run.
Library process means before/after are **17.451/8.139**, **16.766/7.144** and
**24.242/7.352 ms**, or 53.4–69.7% less wall time. The third before session has
substantial extra wall cost and is retained. Actual CPU CLI means are
**17.304/15.409**, **17.499/15.734**, **17.415/15.614 ms**: 10.1–11.0% less wall time.
Declared metadata peak increases **36,728 → 37,248 bytes** from the larger owned
program nodes; decoded peak remains **786,432 bytes**. This is not a memory saving.

## Controls and limits

The write-only cases do not exercise this source-program optimization and are
important controls for host/code-generation variation. They retain mixed sessions
and slow tails. Repeated diagonal library session three is 11.1% slower, repeated
brush library sessions are 0.9–7.0% slower, and dense CLI sessions are 3.8–5.5%
slower. Varied horizontal CLI has a 26.9% slow first session; varied diagonal CLI
has a 30.4% slow second session; varied dense library ranges from 36.2% faster to
20.4% slower. None is discarded or presented as an effect of the coverage check.
All twelve cases' statistical intervals and all 78 raw process exports are retained.

Select this narrow source-read optimization with its exact semantics. It does not
establish a general throughput improvement, a CPU/GPU dispatch policy, physical
memory ceilings, cross-platform results or native p95/p99 input/display latency.
The prior complete-batch versus sequential regression record remains intact.

[Validation](../../../docs/validation/projection-coverage-2026-10-22/validation.json)
records **914 workspace passes** (26 ignores, 85 suites), **126 optimized raster**
and **120 optimized editor** passes, strict all-target Clippy, and release CPU CLI/
benchmark builds. Pixel-oracle, coverage-hole/append/clipping, source-error and
bounded-miss cancellation tests pass; long-list cancellation remains tested at a
point inside the envelope but outside every write. No dependency or wire/file policy
changes. Broader performance work remains open; the next task is Classic UI fidelity.
