# Bounded regional GPU sources: retain the single-view default

The renderer can retain multiple exact clipped source views of a layer under its
existing byte, metadata and entry caps. Keep the original single-view-per-layer
policy as the default. The regional policy is an explicit host option through
`with_region_source_cache(true)`, not a globally enabled acceleration claim.

## Reproduction and equivalent required work

Measured implementation/driver commit: `be6ab2a`. The `benchmarks/results/gpu-region-sources-2026-10-21/manifest.json` (local result)
records full source identity, executable hash, compiler, environment and exact
test. All measurements use the optimized prebuilt ignored library test
`cpu_rendering::display_benchmark::native_multires_process`.

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json
python3 benchmarks/tools/run_native_viewport.py --suite gpu-regions --binary <reported-lib-test-executable> --output <new-result-directory>
```

The four variants are `gpu32`, `gpu32regions`, `gpu64` and `gpu64regions`. Within
each tile edge only source-retention policy differs. Both use the exact same
ordered native composition, wide-integer composite-before-filter reduction, output
dimensions, completed submissions and App native texture acceptance. No timed
readback is substituted for display, and no output work is omitted. CPU scaled
display stays the automatic desktop default, independently of this experiment.

Each sequence includes cold device, renderer, font and document construction and
teardown plus eight frames. Tiny 32-square, dense five-layer 512×256 and sparse
4096×2160 cases grow the same actual size-16 translucent desktop brush preview
by eight points each frame. Uniform 4096×2160 instead publishes eight typed
opacity edits, retaining zero resident raster tiles. Sampling levels are one for
tiny/dense and two for sparse/uniform 4K.

Preflights and postflights after every process session execute independent command
and full-native/direct-box oracles for every variant, including all output pixels,
unchanged authoritative brush state and the uniform case's eight publications.
Required real adapter availability and exact test execution are asserted. This is
completed native texture readiness, not OS event/display latency or physical GPU
memory qualification.

## Work and residency counters

| Dense eight-frame sequence | Single-view 32 | Regional 32 | Single-view 64 | Regional 64 |
| --- | --- | --- | --- | --- |
| Source bytes staged | 20 MiB | 13.469 MiB | 20 MiB | 3.375 MiB |
| Source buffer creations | 1,280 | 832 | 320 | 40 |
| Unchanged source hits | 0 | 418 | 0 | 266 |
| Retained source buffers | 5 | 64 | 5 | 40 |
| Retained source payload | 0.078 MiB | 1 MiB | 0.3125 MiB | 2.5 MiB |

The 32-edge working set exceeds 64 entries. Request-wide protection keeps early
sources and streams overflow rather than continually evicting the whole working
set. At edge 64 all 40 views fit. Native pixel work remains 1,048,576 per dense
sequence: this change does not cache composed output or eliminate ordered dispatch.

Sparse 4K edge-32 source creations fall from 16 to two, but the same changing rows
still stage 1 MiB. Edge 64 already uses one view and stages 1.125 MiB in either
policy. Tiny retains one view in either policy. Uniform edits visit no native pixels
and stage no sources, providing a control where source retention cannot save work.

Retained payload is measured from application buffer sizes, not physical device
residency. Logical allocation reductions and staged-byte reductions do not by
themselves establish faster completed rendering or lower process RSS.

## Timing, raw evidence and limitations

The table gives per-session baseline/regional median ratios at each tile edge;
above one means the regional variant was faster in that session.

| Case | Edge 32 sessions 1 / 2 / 3 | Edge 64 sessions 1 / 2 / 3 |
| --- | --- | --- |
| Tiny | 0.880 / 0.963 / 1.014 | 0.912 / 1.008 / 1.161 |
| Dense five-layer | 1.154 / 1.097 / 0.981 | 0.971 / 1.063 / 1.405 |
| Sparse 4K | 0.972 / 1.240 / 1.480 | 0.978 / 1.733 / 1.009 |
| Uniform 4K control | 0.987 / 0.890 / 1.092 | 0.982 / 0.851 / 1.015 |

Dense edge-64 medians range from 229.97 to 887.53 ms baseline and 223.80 to
631.77 ms regional. The largest apparent sparse-4K improvement occurs at edge 64,
where both variants stage exactly the same bytes and retain one source. Uniform
controls do no source work but also swing materially. These controls and differing
session order undermine attributing the larger ratios to the new cache policy.

Criterion's 20 raw flat samples per variant have baseline/regional medians of
29.30/29.28 ms tiny edge 32, 28.71/29.88 ms tiny edge 64;
582.36/588.95 ms dense edge 32, 172.41/170.91 ms dense edge 64;
45.87/54.20 ms sparse edge 32, 41.90/42.27 ms sparse edge 64;
and 26.51/26.17 ms uniform edge 32, 24.99/25.10 ms uniform edge 64.
These mostly tied samples do not substantiate a general throughput improvement.
Retain the single-view default; the explicit regional option exposes the measured
allocation/upload tradeoff without promising faster display.

Dense process RSS medians are about 28–30 MiB across policies despite different
retained buffer sizes. This is not proof that device retention is free. Dense
edge-32 maxima reach 3.13 s baseline and 3.55 s regional; sparse baseline reaches
1.08 s. All slow observations remain included, along with calibration (not
subtracted). No failed or incomplete baseline is treated as a speedup.

Final session summaries are recorded in `benchmarks/results/gpu-region-sources-2026-10-21/summary.json` (local result). All three
sessions, reversed-order session two, calibration and slow observations are retained.
Exact raw Hyperfine exports are gzip-preserved alongside compact equal-valued JSON
and hash sidecars. Criterion sample/iteration arrays are retained independently.
Diagnostic logs are compressed byte-for-byte rather than truncated.
There are 320 raw statistical samples and 1,020 process observations, including
960 comparisons and 60 calibration observations, all with zero exit status.
The runner checks source/binary identity after each session;
`benchmarks/results/gpu-region-sources-2026-10-21/identity-verification.json` (local result) also checks all final recorded
source blobs against the measured commit and the executable hash.

Completion still waits after every native layer and reduction operation. These
waits are a candidate for the next experiment; this comparison does not isolate
their time or qualify asynchronous scheduling. There are no power,
affinity, thermal or background-load controls. No sample is discarded as an outlier,
and this single Metal host does not establish cross-platform dispatch thresholds.

## Correctness and remaining scope

Real Metal tests cover full working-set reuse, exact independent CPU pixels, one
changed row strip, metadata-only reorder/opacity, moved-layer scope retirement,
old display-frame ownership, request-wide over-cap streaming and cancellation at
every observed dirty-transfer checkpoint. Submitted reads finish before source
identity publication; failures clear partial cache state and leave documents intact.
An initial development check ran before the new test file existed and failed; its
log is preserved and excluded from successful validation. Corrected feature-enabled
invocations execute all 23 Metal tests with required real adapter availability.

Regional views remain dense clipped buffers, not a sparse GPU atlas. Sparse working
tiles, asynchronous transfers, bounded dispatch batching, device reconstruction,
physical pressure admission, native interactive latency and broader adapter
qualification remain open. This experiment does not complete T12/T13/T21/T37 or
the feature roadmap.
