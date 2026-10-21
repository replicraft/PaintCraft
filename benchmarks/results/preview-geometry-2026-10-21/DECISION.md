# Reuse growing smooth-path sample unions in desktop previews

Retain the single-entry geometry cache for longer growing previews. The real desktop
preview preparation/CPU composition replay improves locally for growing and retraced
paths. Tiny process cases are 1–3% slower and remain non-win controls. This decision
does not qualify OS input/display latency, GPU transfers, Windows fidelity or every
workload; the metadata limit is a provisional bound on a declared charge.

## Comparable work and correctness

Both candidates use the same optimized desktop test executable and source. Baseline
constructs preview batches and executes the current full-path checked rasterization;
the candidate calls the desktop CpuRenderer::stage cache used by CPU and GPU preview
consumers. Both compose every complete frame through the same desktop CPU renderer.
Fixture creation, command construction, COW staging, returned frames, cache and
Editor destruction are included. This is an application-method replay, not an OS
window/input trace or a parent executable comparison. The baseline also shares the
refactored scanline/union routines; no older-parent speed claim is made.

The 512x256 white document uses a translucent size-16 smooth brush. Tiny cases have
four one-point extensions; growing/retrace cases have 32 frames of 16 new points.
Growing paths advance through a repeating slope; retraces revisit a bounded region.
The preflight checks each complete frame against uncached execution and scalar
composition, verifies no source tiles were modified, and compares both candidates.
Independent distance-oracle, cancellation, clipped/subpixel, scan-budget, selection,
origin and opacity tests passed before timing. Postflight repeats full-frame checks
after each process session and verifies source/binary hashes unchanged.

## Measurements

Criterion uses 20 raw samples per strategy/case, 300 ms warmup and a one-second
requested measurement interval (actual sampling durations are recorded). Hyperfine
uses 20 runs and three warmups in each of three independent sessions, reversing
strategy order in session 2. All 360 comparison runs, 60 empty-process calibration
runs and 120 statistical samples are retained without analyst exclusions.

| Fixture | Criterion baseline/cached median ms per sequence | Process baseline median ms | Process cached median ms | Paired ratio |
|---|---:|---:|---:|---:|
| Tiny | 8.86 / 6.01 | 17.90–18.79 | 18.29–19.45 | 0.97–0.99x |
| Growing | 45.06 / 22.20 | 58.42–59.47 | 34.19–36.38 | 1.63–1.71x |
| Retrace | 34.12 / 14.03 | 46.16–48.07 | 27.24–29.31 | 1.62–1.75x |

The tiny statistical/process discrepancy is retained. Its Criterion baseline has
wide estimates and does not justify a tiny-case gain or threshold. Empty test
process medians are 11.33–12.49 ms; startup and cold fixture/destruction costs are
material, especially for tiny cases. No calibration time is subtracted. Criterion
also includes cold fixture/destruction, rather than claiming a steady kernel cost.

Reported process RSS medians are 20.48 / 14.84 MiB for growing and 16.20 / 11.48 MiB
for retrace; corresponding maxima are 26.69 / 22.39 and 22.38 / 17.31 MiB. Tiny
cached maximum is worse (11.19 versus 10.88 MiB). These are complete-process
observations on one developer host, not a physical-memory bound or long-session
reclamation result. Neither CPU affinity nor thermal/power/background load was
controlled. No build, test or other benchmark was intentionally concurrent.

## Reproduction and retained evidence

Build with `CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run`.
The manifest records all workspace Rust/manifests/lock hashes and the executable
identity. The source was frozen before measurement; later source commits name that
same source. Run the optimized lib test executable with:

```text
--exact cpu_rendering::geometry_benchmark::preview_geometry_process --ignored --nocapture
```

Set `PAINTCRAFT_GEOMETRY_VARIANT` to `verify`, `criterion`, `baseline`, `cached` or
`calibration`; process cases use `PAINTCRAFT_GEOMETRY_CASE=tiny|growing|retrace`.
Use Hyperfine `--shell=none --warmup 3 --runs 20 --export-json FILE` with explicit
`/usr/bin/env` commands. Reverse candidate order in the middle session and retain
postflight checks. Raw Criterion sample/estimate/config files are in `criterion/`.
Original process exports are byte-preserved in `raw_exports/*.gz`; compact JSON
contains identical parsed values. `raw-exports-sha256.json` records both hashes.
Validation and native smoke evidence accompany the timing records.

Geometry reuse still resolves the complete sample union and stages current source
pixels. Prefix validation, parsing, compound-preview cache churn, fine dirty-region
painting, viewport composition, GPU residency and native queue/display tails remain
open. This is not completion of the incremental-stroke or performance roadmap.
