# Decision: explicit bounded CPU row-band export

## Selection and limits

Adopt the caller-owned bounded pool API and 64-row bands for explicitly requested
CLI parallel rendering. Keep CLI exports serial by default. `--render-workers N`
selects 1..64 workers; N=1 uses ordinary rendering without a pool. Four-worker bands
improve the measured expensive layered process workloads by 1.5–2.9×, but regress
opaque/clipped cases and strongly regress tiny outputs. This is not an automatic
worker-count or crossover policy. Two workers provide a smaller, distinct budget.

64- and 256-row bands are close on expensive cases; neither wins universally.
64 rows provide more independent jobs for caller-selected pools and work well
across these layered fixtures. Do not claim a statistically established universal
granularity win. Retain the safe tile-view implementation as an exact measured
alternative; 64-pixel tiles have substantial metadata/row-span overhead, especially
with many sparse layers. 256-pixel tiles are competitive in dense cases but do
not justify replacing row bands for this bulk consumer.

Do not replace the desktop local-edit cache/GPU path with full parallel frames.
This experiment does not compare them, measure GPU crossover, or prove a priority
scheduler. Full CLI PNG/BMP/JPEG encode/write performance is not measured here;
the shipping export consumer is functionally tested. CPU preparation is the
measured process workload, not whole-command export throughput or display latency.
BF06/T12/T13 remain partial, including interactive interference and shared process
resource admission. A dedicated custom worker implementation is not compared;
these results do not establish Rayon superiority over every alternative executor.

## Implemented work and invariants

`render_in_pool` takes an explicit caller-owned Rayon pool, a checked output region,
a band/tile partition and a tile-view byte budget. The renderer creates no pool,
uses no global Rayon pool, mutates no document/history and holds no document-global
lock. Each job writes disjoint output bytes. Tile jobs use safe borrowed row views,
not a second full pixel surface. Same-pool nested execution reuses its workers.
Cancellation checks precede allocation, continue through setup/layers/rows, and
join private work before errors discard the output. Malformed partitions, output
regions and tile-view admission failures return errors without partial output.

The existing default-v1 RGBA8 row kernel is shared without changing root order,
opacity rounding, offsets, finite clipping or transparent-black starting pixels.
Sources stay immutable sparse COW rasters. Effects, profiles, groups, halos and
multiresolution representations are not part of this implemented flat policy.

The CLI constructs at most the explicitly requested render pool, joins rendering
before codec work and completes encoding/publication afterwards. Its image codec
features do not enable a hidden Rayon codec pool (`codec-features.txt`). The normal
CLI dependency graph excludes Criterion, egui/eframe and wgpu. The default path
retains serial rendering. `export_flat_with` preserves loss acknowledgement, rejects
unknown losses before renderer work and validates complete output before encoding.

## Qualification gates

510 workspace tests pass (20 explicitly ignored manual/review cases). Strict
workspace/all-target Clippy, formatting and the repository 800-line check pass.
Required real Metal parity passes on Apple M1 Pro after the shared row refactor.
The standalone release CLI builds, exposes the new option/version, and its actual
serial/four-worker smoke exports have identical PNG bytes.
New tests cover 1/2/4 workers, clipped/cropped regions and edge tiles, exact layer
order/opacity, unchanged source versions, reentrant same-pool execution, active
cancellation, invalid input and metadata budgets, and tiny regions of huge sparse
canvases. Real CLI tests export layered PCND at 1/2/4 workers, independently decode
PNG, compare all pixels plus a hand-calculated translucent pixel, and reject
worker counts 0/65 without writing. Existing all-alpha independent arithmetic,
canonical PCND and full edit/save/reopen/export tests also pass.

Each process session runs frozen row-copy scalar/full-output preflight for every
candidate. Timed work checks output length and five separately evaluated pixel
probes, consumes and drops the complete output. Probe checks are liveness guards,
not a replacement for the full independent oracle tests.

## Statistical evidence and the new dependency

Criterion 0.8.2 is development-only, pinned with cargo_bench_support/csv_output.
Its optional Rayon analysis and plotting features are disabled, avoiding another
analysis pool (`harness-features.txt`). It was selected for raw sample/CSV export,
not a claim that the harness itself is faster. Divan 0.1.21's public CLI lacks raw
internal-sample export; existing Divan allocation summaries remain useful, but
older span/cache in-process raw-sample gates need follow-up. Their immutable
process evidence is not rewritten or discarded.

Two Criterion sessions use the identical binary/source: forward order, then
exact-filtered reverse case/variant order. Each parameter has 100 flat samples,
1-second warmup and 2-second measurement, default 100,000 bootstrap resamples.
49 parameters per session give 9,800 retained samples. Pool/fixture construction
and full oracle checks are outside these statistical measurements; actual render,
live probes and output destruction are included. Raw JSON and CSV both have 100
samples per parameter and pass finite/positive checks. Per-session 95% median
confidence intervals are retained; no pooled confidence interval is invented.

Pooled medians of normalized raw times/iterations (200 samples per cell), ms:

| Case | Serial | Band64 ×1 | Band64 ×2 | Band64 ×4 | Band256 ×4 | Tile256 ×4 |
|---|---:|---:|---:|---:|---:|---:|
| tiny | 0.0010 | 0.0057 | 0.0071 | 0.0085 | 0.0089 | 0.0079 |
| opaque1 | 0.3525 | 0.4677 | 0.3999 | 0.4730 | 0.3552 | 0.4891 |
| sparse5 | 0.8183 | 0.9137 | 0.6189 | 0.5595 | 0.6082 | 0.5885 |
| sparse20 | 2.4322 | 2.5087 | 1.5400 | 1.0063 | 0.9721 | 1.0489 |
| sparse100 | 11.2612 | 11.4773 | 6.2064 | 3.4413 | 3.4331 | 3.9638 |
| dense5 | 21.0979 | 21.6140 | 10.8277 | 5.7909 | 5.7488 | 5.8832 |
| clipped100 | 0.2228 | 0.3688 | 0.3931 | 0.3938 | 0.3925 | 0.3954 |

The CSV retains all seven variants, session medians and confidence bounds. The
four-worker dense case is about 3.6× faster in steady composition; process results
below include additional real costs. Small/opaque/clipped cases demonstrate that
parallelism itself is not a fast path. No unlike-workload speedup average is used.

## Whole-process evidence

Apple M1 Pro, 32 GiB RAM, 10 logical CPUs, macOS 14.7.2, aarch64; Rust 1.99.0 /
LLVM 23.1.1, release opt-level 3 with line tables and normal release codegen/LTO
settings. Explicit build environment overrides are recorded as unset. Hyperfine
2.0.0 performs three independent forward/reverse/forward sessions, with 20 runs
and 3 warmups per parameter: 2,940 accepted timed processes. There was no competing
benchmark or compilation during accepted timings. Host power, affinity, thermal
state and other background load were not controlled; all outliers are retained.

The prebuilt process constructs its fixture and pool, renders a fixed equal batch,
drops every output, and tears down. Source revision `1d87c23`, binary/fixture/driver
hashes and compiler metadata are recorded. Statistical and process binaries are
distinct and individually hashed. All relevant source identities remain unchanged.

Pooled process medians (60 observations per cell). Ratio is serial / band64×4;
below 1 is regression. RSS is median per-process peak MiB, not allocator/live bytes.

| Case | Renders/process | Serial ms | Band64 ×4 ms | Ratio | Serial RSS | Band RSS |
|---|---:|---:|---:|---:|---:|---:|
| tiny | 4096 | 6.31 | 36.22 | 0.17× | 1.7 | 2.1 |
| opaque1 | 256 | 87.19 | 121.88 | 0.72× | 33.6 | 34.0 |
| sparse5 | 128 | 106.02 | 69.81 | 1.52× | 30.9 | 31.2 |
| sparse20 | 64 | 167.41 | 72.89 | 2.30× | 34.7 | 35.1 |
| sparse100 | 16 | 224.44 | 91.24 | 2.46× | 51.6 | 51.9 |
| dense5 | 16 | 381.04 | 132.57 | 2.87× | 51.5 | 51.4 |
| clipped100 | 256 | 95.36 | 125.18 | 0.76× | 59.5 | 59.8 |

`summary.csv` retains all variants, work-unit throughput, CPU time, cycles,
instructions, min/max/deviation and peak RSS. Dense5 process throughput improves
from about 42 to 121 complete renders/s, including fixture/pool construction.
Dense5 median CPU time is 373.3 → 381.7 ms while wall time falls from
381.0 → 132.6 ms: the gain uses concurrent cores with a small increase in total
CPU work. These are batch renderer rates, not CLI codec throughput or displayed frames/s.

Zero-render native time calibration is retained for every case/candidate. These
are single supplementary observations, not precise startup distributions. The first
tiny serial calibration has a large cold-start outlier (0.18 s); it is kept, not
subtracted or used to normalize the comparison. Tiny-process results are dominated
by startup/dispatch and are an overhead control. Fixture/pool costs materially
affect faster candidates, so a universal startup-below-5% claim is not made. Native
resource logs retain internal render_ns alongside total process observations.

Both band and tile paths allocate one dense output; tile views add only admitted
reference metadata. At 1024², requested tile-view capacities are about 270 KiB for
64-pixel tiles or 64.9 KiB for 256-pixel tiles (56-byte view entries and 16-byte row
references on this target). Actual capacities are checked; allocator overhead,
thread stacks, sources and process RSS are separate. The driver allows 32 MiB of
view metadata; CLI band exports need none. Peak RSS is roughly tied on these band
workloads, not evidence that additional worker stacks are free or that source
residency is globally admitted. No persistent parallel cache is introduced.

## Artifacts and reproducibility

`criterion-forward.tar.gz` and `criterion-reverse.tar.gz` preserve original sample
JSON/CSV, estimates and outliers. Reverse execution logs/argv/order and its inner
manifest are retained. `criterion-evidence.json` identifies the first prebuilt
binary/source/configuration; `sample-validation.json` records completed counts and
hash checks. `criterion-summary.csv` is derived from the preserved samples.

`session-*.tar.gz` retain all original Hyperfine exports, execution/correctness logs,
source hashes and native process resources. Inner timing/resource manifests are
verified before archiving and summarization. Raw qualification status stays
unreviewed; this document is the scoped engineering assessment. Outer
`MANIFEST.sha256` covers all retained files. Test/Clippy/Metal logs, build environment,
normal CLI dependency graph and codec/harness feature graphs are retained.

The exploratory Divan pilot is excluded (no raw samples, fewer repetitions and
no live-probe guard yet). The sandbox dependency-fetch failure is also retained;
Criterion was downloaded with authorized network access before builds/measurements.
There is no failed or incomplete session promoted into the accepted timings.

```sh
CARGO_INCREMENTAL=0 cargo build --release -p paintcraft-render --example partitioned
CARGO_INCREMENTAL=0 cargo bench -p paintcraft-render --bench partitioned --no-run
# Run the emitted benchmark binary with --bench and a new CRITERION_HOME.
python3 -B benchmarks/tools/run_composite.py --suite parallel --output /tmp/new-partition-run --execute
python3 -B benchmarks/tools/run_partition_samples.py --configs /tmp/new-partition-run --binary <prebuilt-bench-path> --output /tmp/new-reverse-samples --order reverse --execute
python3 -B benchmarks/tools/summarize_composite.py /tmp/new-partition-run --output /tmp/new-partition-summary.csv
```

Run phases sequentially, with fresh output directories. Never rebuild or run
another benchmark concurrently. No tools are installed by these runners.

## Remaining work

Interactive queue-inclusive latency, cancellation age/drain under contention,
priority scheduling, codec/plugin/history cross-pool budgets, parallel cache misses,
full CLI codec end-to-end timing, additional geometry/dense scales, other CPU/ISA
hosts and GPU residency/crossover remain unqualified. Older span/cache raw-sample
follow-up, Moka versus the current small LRU table, finer operation invalidation,
history logical/resident ownership, compression/spill and broader formats remain
in the active roadmap. This milestone does not complete BF06 or the full goal.
