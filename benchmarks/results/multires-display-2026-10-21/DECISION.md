# Bounded CPU multiresolution display and native upload readiness

Adopt the exact bounded CPU box-sampling display baseline with bounded coarse-cache
reuse. It enables zoomed-out giant canvases without a full native source surface.
The sparse 4K fixture confirms a 3.12–3.20× process improvement over the existing
native-resolution compositor/cache followed by the same box filter. Tiny views are
effectively tied, and dense multilayer views are mixed/tied against that established
cache. The coarse cache substantially improves dense input over uncached reduction,
but is not universally faster. GPU scaled composition remains unimplemented.

## Source and reproduction

Measured source/runner commit: `d6345efab7ac8cafa9d2e8a8f751a3fc6cf96345`.
The [manifest](manifest.json) records compiler/build, executable, Rust/WGSL/manifests,
runner, environment and exact test identity. [Identity verification](identity-verification.json)
checks frozen source blobs and the executable after all sessions. A later
[test-only follow-up](test-followup.json) adds independent dense recursive-boundary
coverage; production/benchmark code and the measured executable remain unchanged.

```sh
CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo test -p paintcraft --lib --release --no-run --message-format=json
python3 benchmarks/tools/run_native_viewport.py --suite multires --binary <reported-lib-test-executable> --output <new-result-directory>
```

The optimized ignored lib test is
`cpu_rendering::display_benchmark::native_multires_process`. It accepts
`PAINTCRAFT_VIEWPORT_CASE=tiny|dense|large` and
`PAINTCRAFT_VIEWPORT_VARIANT=full|bounded|cached|verify|criterion|calibration`.
The runner checks that the exact test exists and oracle preflights execute once;
required real-adapter information is printed in correctness logs.

All variants stage the same actual desktop incremental brush geometry and commands,
then compute the same ordered-composite/alpha-weighted encoded-sRGB box samples,
install them through App's sampling-aware acceptance/ColorImage path, complete actual
egui managed-texture uploads and destroy results/caches. Each sequence contains eight
frames with eight new points per frame and a translucent size-16 brush. Native device,
renderer, font initialization, document construction and teardown are included.
This is completed texture readiness, not OS presentation or event/frame latency.

`full` retains the real existing `CpuRenderer::render_region` native-resolution cache,
including prefix/regional patch behavior, then directly filters the materialized ROI.
It does not discard previous optimizations to create a weak baseline. `bounded` uses
the correct bounded reducer without coarse content caching; `cached` uses the actual
CPU display path with 32-sample tiles, 4 MiB committed / 1 MiB preview and 128 entries
per partition. This is one bounded baseline configuration, not a proven optimal tile
layout, admission budget or general dispatch threshold.

| Case | Source | Layers | Level / step | Output |
| --- | --- | --- | --- | --- |
| Tiny | 32×32 | 1 sparse implicit white layer | 1 / 2 | 16×16 |
| Dense | 512×256 | 5 dense synthetic layers with varying alpha | 1 / 2 | 256×128 |
| Large | 4096×2160 | 1 sparse implicit white layer | 2 / 4 | 1024×540 |

The million-square capability fixture is separately verified, not timed against an
unsuccessful full-native control. It cannot be used to invent a paired speedup.
None of these fixtures qualifies photographs, groups, halos, profiles or Paint's
historical sampling behavior.

## Correctness and required work

Before and after each process session, independent command execution plus full
scalar ROI composition and direct pixel-by-pixel box summation verify every output
pixel and unchanged authoritative source/ticket. Both paths compose before filtering;
they do not compare a cheaper layer-first approximation against exact output. Shader/
CPU policy, output dimensions and required native backend upload work are identical.

Separate tests cover negative/positive offsets, opacity, hidden RGB, global sample
grid alignment, partial canvas-edge normalization and texture footprints, level/preview
cache isolation, invalid requests, metadata proofs, cancellation and poisoned-cache
fallback. A red/blue two-pixel case explicitly rejects layerwise downsampling. The
follow-up tests dense source partitions whose recursive cuts cross sample cells and
256-pixel source tiles. All source storage and native export policies remain unchanged.

A 1,048,576-square uniform document produces 1024-square output without visiting
native pixels. After changing a 256-square tile, at most 65,536 native pixels are
visited; the expected weighted sample is independently checked. This is a synthetic
work-count/capability result, not huge-document latency or physical memory qualification.

## Timing

Criterion retains 20 flat raw samples per variant/case (180 total), 300 ms warmup and
requested one-second windows. Three Hyperfine sessions each retain 20 timed runs and
three warmups per variant/case; session two reverses all three variants. All 540
comparison observations and 60 zero-work calibration observations remain unexcluded.
Times are milliseconds per complete eight-frame sequence with cold setup/teardown.

| Case | Criterion mean full / bounded / cached | Process medians full, sessions 1–3 | Process medians cached, sessions 1–3 | Paired full / cached |
| --- | --- | --- | --- | --- |
| Tiny | 39.70 / 20.92 / 17.94 | 60.03, 59.44, 58.89 | 60.01, 58.10, 57.45 | 1.00–1.02×; tie |
| Dense | 30.53 / 56.97 / 26.68 | 72.99, 77.09, 73.31 | 73.62, 70.81, 71.72 | 0.99–1.09×; mixed/tied |
| Large | 199.90 / 37.80 / 38.19 | 254.38, 252.97, 257.29 | 80.29, 79.05, 82.50 | 3.12–3.20× |

Tiny statistical gains do not survive process confirmation; they are not promoted.
Dense uncached process medians are 95.28, 95.96 and 96.47 ms, so cached/bounded
ratios are 1.29–1.36× in favor of reuse. The established full-resolution cache remains
competitive. Large uncached medians are 80.19, 76.46 and 77.79 ms; the coarse cache
costs approximately 0–6% there, while the bounded strategy retains the major gain.
The current bounded cache preserves dense reuse rather than claiming universal wins.

Calibration medians are 13.21, 13.41 and 13.23 ms and are never subtracted. Cold GPU/
renderer/font/process costs are material for tiny inputs. Slow observations and
statistical/process disagreement stay in [summary.json](summary.json) and original
exports without an invented cause or selective exclusion.

## Memory and retained cache costs

| Case | Process RSS median range full / bounded / cached, MiB | Maximum RSS full / bounded / cached, MiB |
| --- | --- | --- |
| Tiny | 20.91–21.13 / 20.98–21.79 / 21.09–21.54 | 22.28 / 22.55 / 22.73 |
| Dense | 28.30–29.41 / 25.81–25.84 / 25.65–25.98 | 31.78 / 27.27 / 27.42 |
| Large | 76.62–77.02 / 42.45–44.52 / 42.71–44.55 | 82.75 / 52.00 / 51.50 |

Dense full composition retains 1,048,576 native preview pixel bytes plus 71,952
metadata bytes in four full/prefix entries. The coarse path retains 131,072 pixel
bytes plus 47,360 metadata bytes in 32 entries. Large coarse previews retain 8,192
pixel bytes plus 15,760 metadata bytes in two entries. The uncached reducer retains
no coarse entries. Unused native/coarse entry-table charges remain visible in logs.
These are service weights and peak whole-process RSS, not physical GPU residency,
post-eviction reclamation, global allocator bounds or native pressure/latency evidence.

## Artifacts, validation and limits

Original Hyperfine bytes are retained in gzip with equal-valued compact JSON and
hash sidecars. Criterion samples/estimates, raw logs, full correctness pre/postflights,
source/binary identity and summary are preserved. One Apple M1 Pro developer host has
no affinity/power/thermal/background-load controls. No builds, other tests or other
benchmarks were intentionally concurrent with timing; no timed observation is removed.

762 workspace tests pass (25 explicit ignores), strict workspace/all-target Clippy,
formatting/file limits and release desktop/CLI builds. Sixteen required Metal renderer
tests and three native registration/draw tests remain green. A required-Metal native
worker test admits reduced million-square output under the ordinary worker limit and
keeps the GPU available. GUI tests fit a giant document, restore actual-size view,
preserve history/source and verify partial-edge mesh footprints/clipping. A 30-frame
release window smoke exits zero with no log output; it covers default native startup,
not live giant-canvas OS input qualification.

`excluded-validation/` preserves the pre-measurement cache-level coexistence and
Clippy failures. A note identifies the first compiler corrections whose output was
not saved as a raw log. These collected no timings. The test-only follow-up and its
final complete workspace/Clippy logs are retained separately from the frozen run.

Native GPU scaled composition, tiled output above hardware limits, hierarchical
level refinement, parallel reduction/cache-miss integration, optimal layouts,
asynchronous services, full incremental operation staging, device recreation,
physical all-service budgets and native input/display tails remain incomplete.
Final exports and sampled-composite tools do not consume viewport LOD. Historical
Windows/Paint.NET reference parity and other platforms remain separate gates.
