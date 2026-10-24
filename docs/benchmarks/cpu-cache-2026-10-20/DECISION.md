# Decision: bounded regional CPU output and lower-prefix caches

## Selection

Adopt exact regional composition and bounded output/prefix reuse. Use 256-pixel
output tiles in the desktop CPU fallback, with 16 MiB committed / 4 MiB preview
partitions and 512 entries per domain. All three grids have exact output, but
64/128 grids add key/entry overhead without reducing invalidated pixels for the
current 256-pixel source versions. The 256 grid has the best pooled process median
among cache candidates for these fixtures; close dense-case differences should
not be treated as a confidently measured 128-versus-256 kernel win.

Retain ordinary rendering for one relevant layer: its sparse replay regresses
about 21% with caching. Tiny outputs also bypass the cache conservatively; their
crossover is unqualified. Keep GPU-first dispatch: this experiment does not compare
CPU/GPU transfer or presentation costs. No dependencies are added in this slice.

The global bottom-layer opacity control regresses about 8% and increases process
memory. Accept this documented tradeoff for the local/multi-layer gains; the
current CPU integration does not automatically bypass globally invalidated lower
prefixes. Metadata-aware cost dispatch remains work. Do not call this cache
universally faster, a complete dirty-rectangle renderer or a residency authority.

## Equivalent work and source identity

Every variant runs the same admitted atomic batch-v2 transactions, recorded history
and exact native RGBA8 output. Local edits change alternating translucent pencil
samples in the top layer. Global controls alternate opacity in the top or bottom
layer of five uniform translucent layers. Other fixtures include fully occupied
mixed-alpha tiles and 1/5/20/100 sparse layers with hidden RGB. Viewport output is
384×256 at (25,19) in a 1024², 100-layer document. Other outputs are 1024².
History has the same 32 MiB budget. Outputs are destroyed inside measured work.

All contenders prepare one first frame. Divan excludes fixture/first-frame setup;
process timing includes it, history retention/trim, output assembly and teardown.
Zero-edit calibration therefore includes the first frame/cache preparation; it is
not subtracted. Cold preparation is not represented as a standalone renderer
latency claim. Work-unit rates count completed edit/render steps per whole process,
not display frames per second.

Renderer implementation: `09dafcb`; first replay: `3c05b85`; desktop integration:
`b58b579`; invalidated-prefix control: `890e177`. Timing began from development
snapshots before their logical commits; each config records the then-current HEAD,
worktree state and exact file/binary hashes. First seven cases used the first
replay version. The added bottom-layer case is separately retained under
`invalidated-prefix/`, with its own source/driver hashes. The compositor/cache
implementation is identical across these sets. The final Divan session includes
all eight cases from the extended driver. `divan-evidence.json` identifies it.

## Correctness and resource gates

505 workspace tests pass, with 20 explicitly ignored review/manual cases. Strict
workspace/all-target Clippy, formatting and the 800-line repository check pass.
The required native Metal adapter test passes exact CPU/GPU parity on Apple M1 Pro.
New tests cover regional partition reassembly, a huge sparse canvas, malformed
ranges, cancellation, weak-version lifetime/COW mutation, exact prefix rounding,
lower-prefix invalidation, undo/redo, opacity/visibility/structure/offset changes,
partial viewports, budget eviction, committed/preview isolation and actual desktop
CPU preview/cancel/commit/undo/redo. A poisoned derived-cache worker is safely
retired; subsequent CPU frames recover. Existing independent all-alpha scalar
checks, canonical PCND fixtures and disk edit/save/reopen/export workflow pass.
Each timed process session additionally runs replay equivalence preflight.

A one-pixel edit recomposes 65,536 output pixels, versus 1,048,576 for a full
1024² frame. At grids 64/128/256 it recomposes 16/4/1 tiles respectively; unchanged
lower layers are reused within these tiles. Dirty granularity still follows source
storage versions. The output is assembled into a new dense Vec in every variant.

Cache weights include pixel/table/key capacities plus conservative weak-control
block charges and stay within separate explicit byte/entry limits under eviction.
Weak source identities do not retain decoded source Vecs. Returned output, scratch,
source/history residency, allocator overhead and RSS are separate. Peak RSS below
is the entire process, not the nominal 20 MiB persistent cache budget. A cache can
increase memory even when it greatly reduces CPU composition time.

## Native process measurements

Apple M1 Pro, 32 GiB, macOS 14.7.2, 10 logical CPUs; Rust 1.99.0 / LLVM 23.1.1,
release opt-level 3 with line-table debug info, Hyperfine 2.0.0. Three independent
sessions in forward/reverse/forward variant order, 20 timed runs and 3 warmups per
case/variant/session: 1,920 accepted timed processes in total. No controlled
power/thermal state or CPU affinity. There are outliers; all remain in raw evidence.
No compilation or other benchmark ran concurrently with the accepted timings.

Pooled medians (60 runs per cell); ratio is full / cache, below 1 is regression.
RSS columns are medians of per-process peak resident memory.

| Case | Edits/process | Full ms | Cache256 ms | Ratio | Full RSS MiB | Cache RSS MiB |
|---|---:|---:|---:|---:|---:|---:|
| sparse1 | 256 | 109.18 | 132.64 | 0.82× | 81.9 | 89.7 |
| sparse5 | 128 | 111.05 | 73.78 | 1.51× | 63.5 | 73.2 |
| sparse20 | 64 | 180.12 | 48.30 | 3.73× | 55.5 | 56.1 |
| sparse100 | 16 | 230.41 | 66.98 | 3.44× | 56.3 | 57.8 |
| dense5 | 16 | 396.61 | 74.72 | 5.31× | 54.7 | 55.5 |
| global5 | 16 | 313.22 | 98.99 | 3.16× | 27.2 | 52.7 |
| global-bottom5 | 16 | 306.73 | 330.45 | 0.93× | 27.1 | 62.3 |
| viewport100 | 64 | 136.05 | 56.08 | 2.43× | 43.6 | 48.9 |

`summary.csv` includes all four contenders, min/max/deviation, completed work units
per second, CPU time, cycles and instructions. For example dense5 throughput is
40.3 → 214.1 complete edits/s including process setup. Top-layer opacity benefits
from unchanged lower prefixes; bottom-layer opacity invalidates them and pays
recomposition plus cache maintenance. Memory does not improve uniformly.

## Divan steady edit/render latency

One 100-sample session per parameter, allocations profiled. This excludes initial
cache construction and fixture setup, but includes transactions/history, exact
rendering, output assembly and destruction. Kernel medians and process speedups
have different boundaries and must not be interchanged.

| Case | Full edit/render ms | Cache256 edit/render ms |
|---|---:|---:|
| sparse1 | 0.2866 | 0.4334 |
| sparse5 | 0.7319 | 0.4150 |
| sparse20 | 2.6760 | 0.4839 |
| sparse100 | 11.0500 | 1.1080 |
| dense5 | 20.5900 | 0.7326 |
| global5 | 17.6000 | 4.4200 |
| global-bottom5 | 17.5000 | 19.1200 |
| viewport100 | 1.5190 | 0.4259 |

The full raw table, including allocation/deallocation profiles and outliers, is
`divan.log.gz`. `divan-summary.csv` retains its printed medians without pretending
they have greater precision. This is not interactive input-to-display p99.

## Evidence and reproduction

`session-*.tar.gz` and `invalidated-prefix/session-*.tar.gz` preserve original
Hyperfine JSON/Markdown, execution records, correctness logs and native process
resource observations. Inner SHA-256 manifests are verified before summarization
and archiving. Raw metadata intentionally remains unreviewed; this decision is the
scoped engineering assessment. Outer `MANIFEST.sha256` covers all retained files.
`workspace-tests.log.gz`, `clippy.log`, `metal.log` and `hardware.txt` retain checks.

The sandbox calibration failure is preserved in `excluded-sandbox-attempt.tar.gz`:
macOS blocked `sysctl kern.clockrate` required by native time counters. It contains
no accepted timing. The rerun used authorized native access. Exploratory no-prefix
and prefix pilot Divan logs are retained as excluded, without source/three-session
qualification. The initial desktop validation failed because an 8 MiB test worker
budget omitted the new cache/key reservation; its original log is retained. The
fixture budget was corrected; freshness/released-reservation assertions remain.
Earlier successful integration checks are retained separately from final checks.

```sh
CARGO_INCREMENTAL=0 cargo build --release -p paintcraft-render --example cache_replay
CARGO_INCREMENTAL=0 cargo bench -p paintcraft-render --bench cache_replay -- --sample-count 100
python3 -B benchmarks/tools/run_composite.py --suite cache --output /tmp/new-cache-run --execute
python3 -B benchmarks/tools/summarize_composite.py /tmp/new-cache-run --output /tmp/new-cache-summary.csv
```

A current full suite includes the added bottom-layer control. Existing evidence
can be re-summarized with both this root and its `invalidated-prefix` subdirectory
as positional roots. Compression is lossless and preserves original timing bytes.

## Remaining qualification

No cross-host, CPU-parallel, SIMD, GPU-residency/crossover, multiresolution, group,
effect halo or long-session compressed-history claim is made. Desktop CPU still
assembles/converts/uploads a full image. Single-source shared surfaces, finer
operation dirty regions, global invalidation cost routing, bounded tile/band
parallelism and process-wide residency authority remain. Existing preview staging
still replays brush paths. T12 remains partial and the full backend goal is active.
