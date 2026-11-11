# Complete built-in batch preparation and decoded admission

The editor now compiles a complete built-in batch before replaying decoded paint.
The public execute/session/native/CLI paths use this preparation; the old executor
is retained only in benchmark support and differential tests.

## Preparation and logical sources

The compiler runs the existing geometry, work-limit, clipping, alias and metadata
validation against a private structural document. Painting produces immutable
ordered descriptors instead of changing that document's pixel buffers. Structural
operations update the shadow layer tree, origins, dimensions and selection in
their original order. A replay record captures its original operation index and
resolved layer ID; geometry is constructed once.

Batches containing preparation-time pixel reads retain source-bound logical
programs. Flood fill sees earlier paint, including branches captured by duplication,
old bounds preserved across expansion, and the current origin/selection. Source
metadata is copied fallibly under admission while immutable pixel buffers remain
leased. Batches that only construct writes omit these logical pixel views: their
ordered blending still reads the destination during replay. An exhaustive command
classification must be updated when new operations are introduced. Dependency
scanning and source/model/geometry walks observe cancellation checkpoints.

Empty plans and operations with no indexed writes need no replay record. This
proof uses the same sparse/default index rules as actual raster replay. An indexed
write is not necessarily a semantic pixel change; existing-tile no-ops retain the
normal COW/admission behavior.

## Tile ownership model

The batch model tracks resident coordinates and ownership, without allocating
decoded tiles or reading pixels. Captured input versions require COW on their
first write. A new private version is reusable until duplication shares it; a
later write to a shared version needs another allocation. Deleting a layer or
clearing the canvas releases a simulated version when its final owner disappears.
Expansion preserves partial edge-tile ownership and allocates no pixels by itself.

The model separately tracks:

- Live new versions and their highest simultaneous count.
- Cumulative allocations, which are not replenished by deletion.
- The first operation that establishes the reported maximum peak.

The cumulative count checks the existing staged-pixel allowance during planning.
The peak prices new output capacity in the decoded realm; existing immutable
input payload is already charged separately. A later allocation can reuse capacity
freed by an earlier deletion without resetting the cumulative work allowance.
Native preparation runs this model serially. Unusual external reader/caller races
can make lower-level estimates conservative; this is not a minimal physical plan.

## Reservation, replay and publication

After compilation, the compiler releases logical source/model storage and retains
ordered original paint records and their actual admitted replay programs. It reserves the complete known new decoded
peak before replay. A quota denial therefore admits no decoded prefix, including
when earlier commands logically painted other tiles. Fixed staged-limit failures
are attributed to the operation that crosses that limit; decoded peak denials
carry the operation establishing the reported peak.

Replay starts from the captured base and replays structural commands and compiled
paint records in original order. Each operation keeps the ordinary checked row
kernel and strict reservation semantics. Runtime invariant checks reject leftover
compiled paint instead of publishing a partial schedule. The session still prepares
recorded history and validates its owner/base ticket before atomic publication.
Undo/redo restores recorded results rather than rerunning geometry or fonts.

Allocator failures, caller interference, cancellation and non-paint metadata
pressure may still fail private replay. Ordinary compiled paint reuses its actual
simulation-created index, validates target bounds/default/ordered sparse rules
without allocating, and does not build another index after decoded admission. All private output and unused reservations
are discarded; published state/history remain unchanged. A reservation is not a
guarantee that malloc succeeds or that a caller sharing it cannot consume capacity.

Source/model nodes, model-map/index payload, logical programs, compiler arrays and
reader scratch carry shared metadata accounts and fixed container limits. Known
metadata retries retain their own reservation policy. Flood bitmap/frontier/output and checked stroke unions now use the exact caller
metadata scope as well as their unchanged fixed per-call scratch bounds. Returned
flood spans retain their original account/factory through descriptor construction.
The ordinary compiled-paint replay barrier holds its actual descriptors/indices
while admitting the complete decoded peak; failures retire those owners before a
native capacity wait. Exact previous descriptors and current modeled target classification can share
one immutable index/program in the same original caller factory, before another
index is built. Every operation still simulates COW and blends in order. Unique
programs retain their own index payload and full aligned Arc/program control
accounts; repeated handles do not re-charge shared storage. Distinct programs
can still increase preparation metadata compared with the earlier temporary-index
policy; no speedup or unchanged work/callback count is claimed. This does not provide a complete upfront
geometry/font/mapped-transform/coverage/history/GPU/physical-memory admission plan. The shadow document and immutable
command input retain their existing metadata policies. Native worker scheduling,
fairness and input/display/cancellation latency remain separate qualification work.

## Executed scope

The current grammar covers fill rectangles, pencil, smooth/unsmoothed strokes,
flood, lines/rectangles/ellipses, resolved masks and built-in text, plus existing
layer/canvas/selection metadata operations. Future effects or new command families
need their own preparation and resource rules; this does not qualify them in advance.

Differential tests compare all painting families and 64 mixed structural sequences
with the source-pinned executor, including exact document semantics, aliases,
selection and decoded peaks. Tests cover zero-prefix whole-batch denial, recycled
peak versus cumulative limits, every observed cancellation checkpoint, borrowed
source capture/COW, metadata interference during real private replay, and native
FIFO/file barriers with repriced whole-attempt demand. Earlier geometry failure
tests now assert preparation fails before any decoded admission.

The batch-preparation benchmark separately compares real editor execution and
parent/current CPU-only CLI apply/history/file workflows. Exact library semantics
are cross-checked with independent raw PCND pixel reconstruction, while complete
native archives verify publication identity. Raw statistical/process samples,
metadata/decoded counters, calibration and slow controls remain required before
any performance claim. Physical RSS ceilings and native latency are not inferred
from these correctness or library measurements.

The [completed matched experiment](../docs/benchmarks/batch-preparation-2026-10-22/DECISION.md)
retains 280 statistical samples and 1,860 original process observations. Exact
outputs and decoded peaks match, but diagonal and mixed-source batches regress
substantially and retain more metadata. This is a correctness adoption; reducing
those costs is separate performance work. [Source-pinned validation](validation/batch-preparation-2026-10-22/validation.json)
records executed checks and preserves development failures.

[Exact descriptor sharing](PAINT_PLAN_SHARING.md) now reduces retained payload for
repeated equal plans without merging operations. Original timing evidence remains
pinned to its pre-sharing source. The [matched follow-up](../docs/benchmarks/plan-sharing-2026-10-22/DECISION.md)
retains memory benefits, mixed timings and slow controls separately.


## Joint paint admission checkpoint

Ordinary compiled paint now retains its exact replay index and original metadata
factory before admitting the complete decoded peak. Exact repeated programs can
share immutable descriptors, indices and fully charged program controls. Each
operation still performs its own simulated ownership changes and actual blend.
Flood and stroke scratch retain their original accounts through handoff/use,
with storage released before its charge. The independent per-call bounds remain.

The final local run passes 633 Raster/Editor tests and 892 desktop library tests,
with 36 existing or explicitly gated desktop cases ignored. Strict native
all-target, Main/both-Worker Wasm and Core/Raster/Editor rustdoc checks pass. The
original repeated-stroke 8× bound, explicit 204,800-byte case, and native
257-pixel stroke under 4,096 bytes all pass without raising their limits.

Last-run coalescing preserves the unordered geometric union before descriptor
construction. A separately declared native-shaped square-stroke case uses two
genuine same-profile benchmark artifacts and an independent complete-image
summary. Logical metadata falls from **8,448 to 2,784 bytes (67%)**; decoded
payload remains 524,288 bytes and full pixels match. This case transplants the
native stroke shape onto the benchmark's 1,000×1,000 document; the original
native 257×1 retry fixture separately passes unchanged. The original seven
benchmark cases retain identical hashes, decoded peaks and logical metadata
peaks before/after coalescing. These are accounting observations, not CPU speed,
allocator overhead, RSS, physical-device or release qualification.

Earlier failed attempts are preserved. In particular, retaining every repeated
index exceeded the original 204,800-byte limit; exact shared programs corrected
that cost. Newly admitted row scratch exposed the 4,096-byte native test's
8,448-byte demand; coalescing removes redundant runs instead of raising the cap.
The benchmark setup failure lacked its required fixture environment; the rerun
uses authenticated archived fixtures in a fresh directory.

The aggregate receipt is
`docs/validation/t04-t11-joint-admission-root/root-final.json`
(SHA-256 `c9be9442a7c76550ffb4e3f61dce575d3e372583d40b0687a508a40b70857f3d`).
Genuine before/after artifacts and policies are retained under that directory.
Browser runtime requalification for this newer source set remains separate.
Mapped transforms, fonts, coverage/history bookkeeping, other services/GPU,
allocator/reallocation controls and physical memory remain unfinished. T04 and
T11 remain partial; this is not a complete upfront all-resource request plan.
