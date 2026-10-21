# Bounded resident GPU layer views with completed desktop preview evidence

Retain bounded source-buffer reuse, contiguous dirty row-strip updates and the
empty-transparent-layer bypass. Unchanged dispatched sources upload zero bytes.
Real desktop preview sequences reduce application-staged source bytes substantially;
complete-process gains are smaller and variable. This does not select CPU/GPU
crossover thresholds, a sparse atlas layout, cache-limit optima or native event/display
latency. GPU-first dispatch remains unchanged and complete host readback remains.

## Same required work and executed gates

Both candidates run the optimized desktop lib test executable through production
Editor synthetic pointer ingestion, preview batch construction, checked private
painting, GpuRenderer composition, per-layer completion waits and full host readback.
Each 512x256 sequence has 32 frames of 16 new input points with a translucent size-16
brush. Geometry caching is enabled for both. Source limits are 0/0/0 for streamed and
64 MiB buffer sizes / 4 MiB metadata charge / 64 entries for resident mode. Device,
shader pipeline, source fixture, renderer/cache construction and destruction are
included. A preview sequence publishes no edit or save; committed host-history
recovery is checked separately by the real-device tests.

The cases have one sparse layer, five fully occupied mixed-alpha layers, or 65 dense
layers. The 65-layer case exceeds the entry limit: sources already used this frame
are protected, the first 64 stay resident, and the remaining layer streams. This
avoids sequential-scan eviction churn without claiming an optimal priority policy.
Both modes skip fully transparent implicit layers under the same integer policy.

Before timing, preflight runs tiny/growing/retraced complete frames through both
GPU modes and both geometry strategies, comparing every image against uncached
execution/scalar composition and checking the original source unchanged. Regional
cancellation, clipping, default/hidden RGB, epoch replacement, resource limits,
panic recovery and CPU lease release have independent tests. All nine required
real-GPU tests pass on Apple M1 Pro / Metal; native smoke exits after 30 frames
with an empty log. After every process session, preflight repeats and source/shader/
manifest/executable hashes remain unchanged. A later test-only addition exercises
cross-frame eviction when a formerly used source becomes invisible; production and
measurement-driver source remain frozen to the reproduction commit.

## Observed work and time

Criterion retains 20 flat samples per case/mode (120 total), 300 ms requested warmup
and a one-second requested interval; slow sequences extend sampling to retain 20
samples. Hyperfine retains three sessions, 20 runs and three warmups per command,
reversing mode order in session 2. All 360 comparison and 60 empty-process calibration
observations are included. No analyst exclusions were applied.

| Sequence | Source bytes staged, streamed / resident (MiB) | Criterion median ms, streamed / resident | Process median ms, streamed / resident across sessions | Paired streamed/resident ratio |
|---|---:|---:|---:|---:|
| Sparse one layer | 16.00 / 8.25 | 153.66 / 140.43 | 88.70–97.09 / 85.44–92.23 | 1.02–1.05x |
| Dense five layers | 80.00 / 10.25 | 178.97 / 134.95 | 161.28–185.45 / 142.73–176.65 | 1.05–1.19x |
| Dense 65 layers | 1040.00 / 48.00 | 1180.83 / 893.00 | 1221.72–1462.89 / 853.90–1425.58 | 1.03–1.56x |

The byte counts come from actual source staging in successful verified growing
sequences, not hardware bus counters or a prediction from cache capacity. Full-width
row strips intentionally transfer more than the minimum dirty rectangle. Fewer
source bytes do not imply proportional frame gains: private raster work, command
preparation, ordered dispatch, completion and readback remain. The sparse gain is
small; the 65-layer timing ratio varies greatly across sessions. CPU affinity,
thermal/power state and other system/GPU load were not controlled, and no cause for
the variation is inferred. No build, test or other benchmark was intentionally
concurrent. Hyperfine flagged a first *timed* resident 65-layer run at 2.474 s after
warmups; it remains included. Nothing was rerun to remove an unfavorable sample.

Empty-test-process medians are 11.80–12.67 ms and are not subtracted. They omit actual
GPU setup; cold adapter/device/pipeline construction is included in each measured
sequence, so these values are not hot kernel latency, GPU timestamp duration,
per-frame FPS or input p95/p99. Statistical and process tiers have different observed
levels, retained rather than reconciled through subtraction.

Reported RSS medians (streamed/resident) are 27.37/26.84 MiB, 31.74/30.30 MiB and
64.84/61.84 MiB. Dense5 resident maximum is higher (36.20 versus 35.59 MiB). Driver
allocation/UMA accounting, buffer-size limits, metadata charge and process RSS are
different quantities. There is no physical-memory bound or general RSS-gain claim.

## Reproduction and preserved evidence

Build the native desktop lib test with the command in manifest.json and run its
recorded executable using:

```text
--exact cpu_rendering::geometry_benchmark::preview_geometry_process --ignored --nocapture
```

Set `PAINTCRAFT_GEOMETRY_BASE=sparse1|dense5|dense65`,
`PAINTCRAFT_GEOMETRY_GPU=streamed|resident`, `PAINTCRAFT_GEOMETRY_CASE=growing`, and
`PAINTCRAFT_GEOMETRY_VARIANT=cached`. `VARIANT=verify` runs full-image/source checks;
`VARIANT=gpu-criterion` compares both GPU modes statistically for the selected base;
`VARIANT=calibration` creates no fixture/device. Use Hyperfine
`--shell=none --warmup 3 --runs 20 --export-json FILE` with explicit `/usr/bin/env`
commands and reverse session 2. A real adapter is mandatory; skipped/model tests
cannot qualify this record. Every replay prints its adapter and source-stage bytes.

Original process exports remain byte-identical in raw_exports/*.gz; compact JSON
has identical parsed values and both hashes are recorded. Criterion raw samples,
config/estimates/outliers, correctness/resource/validation logs and native smoke
are retained. One pre-timing validation fixture was originally an empty transparent
layer, which the new bypass correctly omitted; it was seeded with a resident hidden-
RGB pixel to exercise warm strip updates independently. That failed counter fixture
log is archived under excluded-validation; no timing or pixel mismatch was excluded.

Sparse GPU working tiles/atlas layout, asynchronous region-only transfers/readback,
output residency/direct display, CPU/GPU operation thresholds, physical all-service
budgets, device/window recreation and native queue/input/display tails remain open.
The source limits are provisional guards and these local results do not complete
T12/T13/T21/T37 or the feature roadmap.
