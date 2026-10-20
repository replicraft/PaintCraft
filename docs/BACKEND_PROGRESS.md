# Backend engineering progress

## Initial audit: 2026-10-19, revision 95db5d0

The backend work takes priority over the pending Classic toolbar work. The user
requests correctness and runnable workloads first, then measured compositing,
history resources, brushes, rendering refinement and format qualification.

| Area | Executable implementation | Gaps and immediate implications |
|---|---|---|
| Storage | Finite sparse 256×256 straight RGBA8 tiles; immutable Arc COW snapshots; staged allocation admission | Resident Vec tiles only; no logical store/decoded leases, tile summaries or compression. A tiny COW edit copies a complete tile. |
| Document | Immutable flat ordered raster layers, metadata and distinct document/state identities | No groups, effects, profiles or general selection masks; future validity keys must include these when implemented. |
| Transactions/history | Typed v1/v2 batches, atomic private staging, epoch guards, recorded undo/redo, memory accounting and explicit trimming | Linear snapshots with shared tiles; redo branches are truncated. No recorded patch deltas, compressed tiers or disk spill. |
| CPU rendering | Exact integer ordered composition, clipping, opacity and cancellation | Full output allocation per render, scratch-row copying per layer, per-pixel processing of implicit transparent spans; no dirty cache. |
| GPU rendering | Matching integer compute compositor with qualified test cases and CPU fallback | Fresh staging and readback boundary; no persistent tile residency or device epochs. No performance crossover measurement. |
| Brush | Fixed-point antialiased round coverage, separate preview staging and captured stroke options | Preview replays the path and recomposites a full frame. Pressure/flow/media policies and incremental coverage remain incomplete. |
| Scheduling | Bounded Rayon executor with owner-scoped handles, admission, cancellation and stale-result guards | No priority scheduling, GPU completion authority or interactive latency trace. |
| Formats/CLI | PCND canonical fixtures, bounded readers, image PNG/BMP/JPEG, native save/recovery pipeline, real Clap commands | Other requested formats and source-preserving foreign rewrites remain unimplemented; profile/metadata losses are explicit. |
| Tests/benchmarks | Pixel tests, v1 oracle vectors, GPU equivalence, CLI and GUI workflows, recovery failure tests | No tracked application benchmark results at audit start. Timing and resident-memory claims need new evidence. |

Retain these implementations and the exact `paintcraft.core/default-v1` arithmetic.
No correctness defect is asserted solely from a missing optimization. The row-copy
and full-frame costs above follow directly from `render_cancellable` and desktop
`sync_texture`; their latency impact must be measured before selecting a contender.

The initial workspace test run passed 488 tests (20 explicitly ignored). The
canonical fixture/oracle tests establish existing v1 behavior, not historical
application parity, compressed-history correctness or unimplemented formats.

## First active slice: CPU compositor row spans (T12 / BF29)

Compare the retained scalar row-copy renderer with a read-only borrowed-tile-span
renderer. Skip implicit transparent runs and specialize opaque runs without
changing ordered source-over or opacity rounding. Sparse authoritative storage
stays unchanged; the public full-render result remains an owned dense buffer.
A sparse layer is not a contiguous whole-image surface, and alpha-zero hidden RGB
must not leak into the transparent-black flat backdrop. A borrowed whole-image
shortcut therefore requires a separate surface contract and is not claimed here.

Use deterministic 1/5/20/100-layer sparse fixtures, dense translucent layers,
opaque images, tiny images and clipped layers. Measure steady-state rendering with
Divan, and complete prebuilt driver processes with the existing Hyperfine protocol.
The frozen baseline lives only in benchmark support. No production fallback or
shipping CLI benchmark command is introduced.

Dirty-region/prefix caches, viewport-only composition, parallel partitions,
SIMD, mipmaps, GPU residency/crossover and adaptive compressed disk-backed history
remain required subsequent slices. Neither this audit nor a renderer benchmark
completes those gates.

## First slice delivered

The borrowed-span CPU compositor is implemented with independent all-alpha/opacity
checks, COW/edge/hidden-RGB tests and an on-disk editing/save/export workflow.
493 workspace tests, strict Clippy and a required real Metal adapter parity test
passed. The final dense render allocation remains; one 4 KiB scratch-row allocation
is removed at 1024-pixel width. No source tiles are copied for reading.

[The local decision and raw evidence](../benchmarks/results/cpu-spans-2026-10-19/DECISION.md)
record Divan and three Hyperfine sessions per workload. Sparse process medians
improve 5.5–11.3×; the implicit opaque-background case improves 15.6×. Dense
mixed-alpha composition is inconclusive (about 1.4% slower by pooled process
median). These are full CPU composition results on an M1 Pro, not GPU, frame-tail,
reference application parity or large-document residency qualification.

Next priority is regional composition and dependency-aware reuse for localized
edits, followed by bounded parallel partition experiments. These must retain the
committed cache across preview cancellation and validate structural/opacity/offset
changes. Full history tiering and broader format work remain in the user's backend
request and the original roadmap; this milestone does not reduce that scope.

## Second slice: exact regions and bounded composite reuse

`render_region` now allocates only the requested native-resolution rectangle.
The output cache stores aligned 64/128/256-grid candidates and an exact ordered
prefix below the uppermost relevant layer. Weak immutable source versions detect
changes without pixel hashes or strong references that pin decoded source tiles.
Committed and preview caches have separate byte/entry budgets, so preview
cancellation preserves committed entries. Source storage remains sparse COW.

The desktop CPU fallback now retains this cache across jobs; GPU-first dispatch
and stale document/epoch/gesture rejection are unchanged. Single relevant layers
and tiny outputs bypass caching. A small stroke in the tested 1024² layered image
recomposes 65,536 pixels, with the lower stack reused, instead of recompositing all
1,048,576 pixels. Invalidation currently follows changed storage tile versions;
smaller output tiles do not yet produce finer brush dirty rectangles.

505 workspace tests pass (20 explicitly ignored), strict workspace/all-target
Clippy passes, and required native Metal parity passes. New coverage includes
partial/huge-canvas regions, source version lifetime, exact prefix rounding,
property/structure changes, undo/redo, budget eviction and preview cancellation.
The stale-worker test's artificial 8 MiB budget now includes the new cache/key
allowance; its stale-presentation and released-reservation assertions remain.

See [the implementation contract](REGIONAL_COMPOSITOR.md) and
[the measured cache decision](../benchmarks/results/cpu-cache-2026-10-20/DECISION.md)
for timing, memory and limitations. CPU frames still assemble and upload the full
dense image. Parallel partitions, finer operation invalidation, GPU residency,
mipmaps and history compression/spill remain incomplete; T12 stays partial.

## Third slice: bounded CPU band/tile partitions and CLI export

The renderer now accepts a caller-owned Rayon pool, using disjoint horizontal
bands or safe borrowed tile-row views. Tile view metadata has its own checked
capacity budget; no second dense output or document-global lock is introduced.
Cancellation discards private output after active jobs join. Same-pool nested
execution is tested. The CLI's explicit `render --render-workers 1..64` uses
64-row bands for parallel exports and retains serial rendering by default.
An export renderer callback preserves loss acknowledgement and validates complete
output before encoding/writing; unknown loss codes now fail before renderer work.

510 workspace tests, strict all-target Clippy, formatting, file lengths and real
Metal parity pass. New integration tests independently decode layered PCND exports
at 1/2/4 workers, check a hand-calculated pixel, and reject invalid worker counts
before writing. Region, metadata budget, cancellation and COW identity tests apply.

The evidence audit found Divan 0.1.21 has no raw internal-sample CLI export. Keep
its existing allocation summaries and archived Hyperfine process evidence, but do
not call the raw-sample gate complete. Criterion 0.8.2 is added only for statistical
benchmark consumers with raw JSON/CSV; optional parallel analysis/plotting are off.
A follow-up must add raw in-process samples for the older span/cache choices too.
This is an explicit remaining gate rather than a rewritten historical artifact.

See [the partition contract](PARALLEL_COMPOSITOR.md) and
[BF06 measurements](../benchmarks/results/cpu-parallel-2026-10-20/DECISION.md).
The desktop cache/GPU path keeps its existing dispatch; bulk pool throughput does
not prove interactive responsiveness or scheduling priority. Global CPU/resource
coordination, parallel cache misses, finer invalidation, history compression/spill
and remaining format/roadmap work are still required.

## Fourth slice: recorded cold history and decoded ownership

Older history states can now hold immutable metadata plus shared independent tile
records. Solid and sparse default-relative payloads release decoded tile Vecs;
full-extent tiles retain their original raw allocation. Weak decoded caches and
explicit reader leases keep restoration reusable without pinning packed pixels.
CRC checks guard reconstructed payloads. See [the ownership and failure
contract](HISTORY_STORAGE.md) for exact semantics and budget limitations.

Undo/redo/seek prepare the recorded target before publishing its cursor/document/
epoch. Failed restoration is visible to the UI and retryable. Editing after a cold
restore keeps the preceding state hot. Automatic pressure work caps new tile scans
and complete packed entries before ordinary retention trimming; the two-state floor
and reader-held bytes make retention a soft policy, not a physical-memory cap.

523 workspace tests (20 ignored), strict all-target Clippy and real Metal parity
pass. Tests exercise decoded release with live readers, full recorded navigation,
metadata/selection/off-canvas/hidden RGB, checksum corruption, cancellation,
admission failures, raw fallback, bounded work, table shrinking and epoch overflow.
The release harness compares identical timelines at explicit cold-age checkpoints;
these checkpoints are distinct from the automatic pressure trigger.

Global logical tile authority, previous-version photographic deltas, LZ4/Zstd,
disk spill/segment GC, background priorities, history branches and interactive
latency/resource qualification remain. T04/T07/T11 remain partial.

The [local history decision](../benchmarks/results/history-packing-2026-10-20/DECISION.md)
retains 1,200 process measurements, 800 forward/reverse internal samples and 20
separate phase traces. Sparse 1,000-edit/32-checkpoint history lowers median peak
RSS from 253.5 to 57.2 MiB while increasing complete lifecycle time by about 50%.
Full-extent noise does not save memory and costs roughly twice as much time.
Checkpoint tails and immediate reader-release RSS are reported separately. No
universal interval, historical accounting-regression or interactive latency claim
is made. Release CLI PCND creation and matching serial/four-worker exports pass.

## Fifth slice: independent LZ4/Zstd history blocks

Explicit history compression policies now compare LZ4 and Zstd levels 1/3 with
the raw/solid/default-relative records. Only a smaller encoded representation is
retained. Restores use an admitted fixed-size tile, exact-frame validation and CRC;
no decoded source stays strongly pinned by a compressed record. One scratch buffer
and codec context are reused per compaction. Conservative native headroom and
observed context sizes remain distinct from physical allocator enforcement.

531 workspace tests (20 ignored), strict Clippy and native Metal parity pass.
New cases cover compressed source release, full RGBA navigation, native frame
bounds/truncation/trailing data, corruption, insufficient staging/restoration,
solid/raw/small-patch fallback and cancellation before/after codec execution.
The default remains raw/patch until the comparative evidence supports a choice;
no disk spilling, background priority or complete physical-budget claim is made.

The [codec decision](../benchmarks/results/history-codecs-2026-10-20/DECISION.md)
retains 1,800 process measurements, 1,200 internal samples and 30 phase traces.
LZ4 lowers structured256 median peak RSS from 66.6 to 48.5 MiB but increases the
complete lifecycle from 62.8 to 87.8 ms. Zstd stores fewer bytes but costs much more
CPU and does not yield the lowest RSS. High-entropy controls retain raw allocations
and expose failed-compression overhead. No codec is selected globally.

A follow-up failure audit reproduced dead weak aliases accumulating after a later
tile failed restoration. Failed restores now reclaim those private aliases while
leaving cursor/document authority unchanged. A 128-retry regression passes; the
workspace now passes 532 tests (20 ignored) and strict Clippy. The codec measurements
remain explicitly frozen to 417c0ed, before this correctness follow-up.
