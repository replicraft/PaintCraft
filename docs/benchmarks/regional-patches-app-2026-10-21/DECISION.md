# Desktop confirmation of regional compositor reuse

Retain the current single-layer bypass and 256-pixel output cache with exact region
patching. This desktop consumer comparison confirms dense multilayer preview gains
relative to full-frame composition. Sparse multilayer results are mixed and RSS is
higher with caching. No universal speed/memory gain, native event/display latency,
GPU transfer result or improvement over the previous cache implementation is claimed.

## Actual consumer and equivalent work

The optimized desktop lib test executable runs production Editor pointer ingestion,
preview batch construction, CpuRenderer::stage checked private rasterization and
CpuRenderer::render. Both candidates retain the same incremental geometry cache.
The full control disables only the derived composite cache; the regional candidate
uses the production 256-pixel policy. Both construct the renderer and consume/destroy
all complete frame buffers, input documents, history and caches inside each process.
It is an application-method replay, not an OS window/input trace or frame-rate test.

Each 512x256 fixture has one sparse layer, five sparse layers, or five fully occupied
mixed-alpha layers. The same translucent size-16 brush adds 16 input points in each
of 32 frames. Fixture parsing/construction and teardown are included. Preflight checks
every complete tiny/growing/retraced frame for both compositors against uncached
execution and scalar full output, then verifies the original source image unchanged.
Every session repeats regional full-frame preflights. Source/executable hashes are
checked after each session. Core statistical/grid evidence remains separate in
[the compositor decision](../regional-patches-2026-10-21/DECISION.md).

## Process observations

Three sessions use 20 runs and three warmups per command, with order reversed in
session 2. All 360 comparison runs and 60 empty-test-process calibration runs remain.
No analyst exclusions were applied. CPU affinity/thermal/power/background load was
not controlled; no build, test or other benchmark was intentionally concurrent.

| Case | Full median ms across sessions | Regional median ms | Paired full/regional ratios |
|---|---:|---:|---:|
| Sparse one layer, bypass | 34.65–36.73 | 34.53–36.59 | 1.006, 0.996, 1.004 |
| Sparse five layers | 46.71–49.28 | 39.67–59.94 | 1.177, 1.168, 0.822 |
| Dense five layers | 117.27–125.14 | 62.15–81.43 | 1.887, 1.798, 1.537 |

The single-layer result is a tie. Dense gain persists across sessions; sparse
session 3 reverses the earlier gains and is retained. There is no causal diagnosis
of that drift. Empty process medians are 10.89–14.72 ms and are not subtracted.
Fixture/geometry/staging/cache/output costs remain in complete-work observations;
they must not be interpreted as compositor kernel or interactive tail timings.

Hyperfine-reported process peak-RSS medians for sparse5 are 15.45 / 18.91 MiB
(full/regional), maxima 20.66 / 27.31 MiB. Dense5 medians are 22.00 / 24.18 MiB,
maxima 25.16 / 29.31 MiB. These memory costs support no across-workload RSS reduction
claim; persistent cache accounting, decoded leases and process RSS remain distinct.

## Reproduction and scope limits

Build `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release
--no-run --message-format=json` and select its recorded executable. Run:

```text
--exact cpu_rendering::geometry_benchmark::preview_geometry_process --ignored --nocapture
```

Set `PAINTCRAFT_GEOMETRY_VARIANT=cached`, `PAINTCRAFT_GEOMETRY_CASE=growing`,
`PAINTCRAFT_GEOMETRY_BASE=sparse1|sparse5|dense5` and
`PAINTCRAFT_GEOMETRY_COMPOSITOR=full|regional`. `VARIANT=verify` runs independent
full-frame/source checks; `VARIANT=calibration` runs no fixture. Use Hyperfine
`--shell=none --warmup 3 --runs 20 --export-json FILE` with explicit `/usr/bin/env`
commands and reverse the middle session. Manifest/source/binary identities,
validation logs and exact commands accompany raw data. Original exports are retained
byte-for-byte in raw_exports/*.gz, with equal-valued compact JSON and both hashes.

This confirmation covers CPU preview staging/composition. GPU-first display still
has separate residency/upload/readback work. Small-image crossover, globally dirty
cost routing, compound/growing raster reuse, operation-level exact rectangles,
parallel misses, multiresolution, all-service memory bounds, fairness and native
queue/input/display tail qualification remain open. The full roadmap is not complete.
