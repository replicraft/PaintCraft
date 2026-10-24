# Ordered paint admission and row replay

Retain the shared operation-level plan for its full-operation tile admission and
ordered scalar results. This local comparison supports horizontal-pencil,
brush-run and partial-replacement replay improvements. Diagonal pencil is a
non-win control: Criterion is slower and process results vary around a tie.
This does not complete the native brush-latency or general batch-planning gates.

## Comparison and scope

Both strategies are in the same benchmark binary. The reference calls the direct
incremental raster APIs; the candidate constructs PaintPlan descriptors, indexes
them by tile, reserves capacity, and replays contiguous rows. Fixture creation,
base cloning, tile allocation and result destruction are included where applicable.
Every process validates exact raster equality for all cases before its timed loop
of 100 operations. Coverage replacement uses an independent integer reference
formula. This is not a parent desktop executable comparison or an input trace.

Criterion uses 20 raw samples, 300 ms warmup and one second measurement per case.
Hyperfine uses three sessions, 20 process runs and three warmups per command;
session 2 reverses strategy order. All 480 process observations and 160 Criterion
samples for this candidate are retained. No analyst outlier exclusions were made.
The manifests record source/binary hashes, tool versions and the local macOS arm64
environment. The CPU-brand sysctl was restricted; no CPU-brand inference is needed
for these source/workload-scoped results.

| Fixture | Criterion median incremental / planned, microseconds | Process median incremental / planned, milliseconds across sessions | Paired process ratio |
|---|---:|---:|---:|
| Horizontal pencil | 77.34 / 45.54 | 12.67–13.38 / 8.74–8.98 | 1.41–1.53x |
| Diagonal pencil | 72.38 / 78.49 | 12.65–12.78 / 12.56–13.31 | 0.96–1.01x |
| Brush runs | 166.54 / 143.12 | 20.99–21.91 / 18.23–19.30 | 1.14–1.15x |
| Partial replacement runs | 2298.33 / 53.85 | 229.12–229.89 / 8.44–9.30 | 24.71–27.15x |

Ratios divide each session's incremental median by its planned median. The large
partial-replacement ratio concerns a synthetic constant-coverage replay fixture;
it is not a claim that the complete native eraser is 25x faster. Parsing, native
input, brush geometry, fonts, history publication, preview scheduling and display
latency are outside the replay comparison. Samples, estimates, process metrics and
session details are preserved alongside this decision; `summary.json` derives
medians directly from the raw measurements.

Hyperfine-reported process peak RSS medians/maxima are also retained. Diagonal
planned replay reached 13.45 MiB versus 10.17 MiB for its incremental control, so
there is no across-workload RSS reduction claim. These process observations do not
establish decoded-budget enforcement as a physical/RSS bound or qualify long
sessions and native pressure.

The [matched parent/current CLI comparison](../paint-plan-cli-cpu-2026-10-21/DECISION.md)
confirms the integrated apply path separately, including geometry, history and IO.
Its smooth-eraser ratio is 1.66–1.73x; native input latency remains unqualified.

## Preserved earlier candidates

The [first prototype](../paint-plan-2026-10-20/manifest.json) and
[grouped lookup prototype](../paint-plan-grouped-2026-10-20/manifest.json) preserve
all measurements, including their diagonal/brush slowdowns. Their manifests map
`raster/paint.rs` to the archived prototype source override; other source hashes
match the final comparison. They are superseded, not dropped from the evidence.
No comparison data from those candidates is combined with this candidate's table.

Original Hyperfine exports are preserved byte-for-byte inside `raw_exports/*.gz`.
The corresponding `.json` files contain identical parsed values in compact form
to respect the repository's 800-line limit. `raw-exports-sha256.json` records both
representations; no metric, observation or metadata field was removed.

## Reproduction

Build with `CARGO_INCREMENTAL=0 cargo bench -p paintcraft-raster --bench paint_plan
--no-run`, then run the reported executable with `--bench --noplot` for Criterion.
Its process mode is `process CASE incremental` or `process CASE planned`; CASE is
one of the fixture names in `summary.json`. Run those commands with Hyperfine
`--shell=none --warmup 3 --runs 20 --export-json FILE`, alternating command order
between independent sessions. Keep sample files and process destruction costs.

Before claiming broader gains, qualify native sustained input/interference,
changing previews, other datasets/platforms, plan metadata admission and general
multi-operation/effect planning. Retain the diagonal cost in future decisions.
