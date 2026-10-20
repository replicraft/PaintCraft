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
