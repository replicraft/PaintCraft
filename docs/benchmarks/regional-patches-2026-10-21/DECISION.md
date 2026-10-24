# Exact changed-region patches within cached output tiles

Retain 256-pixel output tiles and the existing single-layer bypass. Regional source
identities and private output patches reduce a local one-pixel recomposition from
65536 to 4096 pixels without multiplying output cache entries. Smaller output grids
are not consistently faster. This experiment compares current full composition and
current cache configurations; it does not measure a speedup over the previous
whole-tile-key implementation or qualify native input/display/GPU latency.

## Scope and correctness

All variants execute the same atomic Session edits, recorded history and native
RGBA8 output. Sparse and dense local edits alternate translucent pencil samples.
The global control changes bottom-layer opacity; viewport output covers 384x256
in a 1024-square 100-layer document. Other outputs are 1024-square, with one/five
layers. Each sample constructs the fixture and cache, prepares a first frame,
executes eight edit/render steps and destroys results/history/cache. The serial
compositor creates no Rayon pool. No compilation is timed.

Before timing and after each process session, the preflight runs every existing
replay case/variant through twelve edits, comparing each complete output against
full regional composition. Independent scalar compositing, geometry, clipping,
hidden RGB, opacity, COW/admission, raw import/restore, origin/grid alignment,
cancellation and undo/redo tests pass. Revision saturation bypasses caching rather
than reusing stamps; dependency tokens retain no decoded capacity. Frozen source
and executable identities are verified after every session. The source is
reproducible at d53e744, before the later desktop confirmation driver extension.

## Complete-work measurements

Criterion uses 20 flat samples per case/variant, 300 ms warmup and a requested
one-second interval; slow cases extend sampling duration to preserve all samples.
Its 400 raw samples include construction and destruction. Hyperfine uses three
sessions, 20 runs and three warmups, reversing variant order in session 2. All
1200 comparison observations and 240 zero-edit calibration observations are kept.
No analyst exclusions or outlier removal were applied.

| Case | Criterion full / tile64 / tile128 / tile256 median ms | Full process median ms across sessions | tile256 process median ms | Paired full/tile256 ratio |
|---|---:|---:|---:|---:|
| Sparse one layer | 3.61 / 9.22 / 7.47 / 6.43 | 5.54–6.32 | 6.81–7.94 | 0.79–0.82x |
| Sparse five layers | 13.05 / 22.51 / 12.25 / 8.53 | 9.83–10.59 | 8.57–9.04 | 1.09–1.20x |
| Dense five layers | 272.54 / 64.70 / 56.38 / 61.21 | 211.15–219.58 | 46.37–48.35 | 4.37–4.64x |
| Global bottom opacity | 189.95 / 211.15 / 174.27 / 178.32 | 155.91–211.96 | 174.76–220.31 | 0.76–1.20x |
| Viewport, 100 layers | 19.23 / 16.89 / 12.87 / 12.38 | 21.07–29.95 | 14.15–27.45 | 1.09–1.49x |

The single-layer cache control regresses and supports keeping its desktop bypass.
Tile256 wins the sparse-five cached comparisons; tile128 and tile256 have mixed or
close dense/viewport results, so there is no universal finest-grid winner. The
current default remains unchanged. These ratios support scoped cache-versus-full
choices, not a claim that the new patch mechanism alone caused the ratios.

Host variability is substantial: session 3 global/viewport/calibration timings
rise sharply, and the global-control ratio changes sign. That complete session
is retained. No global-edit improvement, causal diagnosis, kernel crossover or
statistically confident near-tie ordering is claimed. Zero-edit calibration includes
fixture/first-frame/cache construction and teardown: tile256 medians are
4.90–10.25 ms versus 2.70–7.39 ms for full. Cold costs can dominate sparse cases;
calibration can exceed a comparison run under the observed drift. Nothing is
subtracted. Per-process RSS medians/maxima are retained in summary.json and do not
establish a physical-memory bound or long-session reclamation result.

## Reproduction and remaining gates

Build the example and `regional_reuse` Criterion target using the commands recorded
in manifest.json. Run the bench executable with `--bench --noplot`. Run the example
with `verify`, or `VARIANT CASE 8`, where VARIANT is full/tile64/tile128/tile256.
Zero-edit calibration is `VARIANT sparse5 0`. Use Hyperfine
`--shell=none --warmup 3 --runs 20 --export-json FILE`, reversing order in session 2.
Original exports remain byte-identical in raw_exports/*.gz; compact JSON has equal
parsed values and both hashes are recorded. Criterion samples/configs/estimates,
pre/postflights and validation logs are retained. Disk-exhausted builds before
measurement are archived separately as excluded build failures, not timing runs.

The [desktop consumer confirmation](../regional-patches-app-2026-10-21/DECISION.md)
checks actual preview preparation/CPU composition separately. The original
whole-tile-identity decision stays frozen in its own artifact directory. GPU
texture validity/residency, multiresolution, effect halos/groups, finer operation
rectangles, parallel cache misses, all-service memory admission and native
queue/input/display tail qualification remain open.
