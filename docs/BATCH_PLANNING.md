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
only ordered original paint records. It reserves the complete known new decoded
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

Allocator/index failures, caller interference, cancellation and fresh metadata
pressure may still fail private replay. All private output and unused reservations
are discarded; published state/history remain unchanged. A reservation is not a
guarantee that malloc succeeds or that a caller sharing it cannot consume capacity.

Source/model nodes, model-map/index payload, logical programs, compiler arrays and
reader scratch carry shared metadata accounts and fixed container limits. Known
metadata retries retain their own reservation policy. Metadata is not reserved
jointly with decoded tiles, and there is no complete upfront geometry/font/flood/
history/GPU/physical-memory admission plan. The shadow document and immutable
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

The [completed matched experiment](../benchmarks/results/batch-preparation-2026-10-22/DECISION.md)
retains 280 statistical samples and 1,860 original process observations. Exact
outputs and decoded peaks match, but diagonal and mixed-source batches regress
substantially and retain more metadata. This is a correctness adoption; reducing
those costs is separate performance work. [Source-pinned validation](validation/batch-preparation-2026-10-22/validation.json)
records executed checks and preserves development failures.

[Exact descriptor sharing](PAINT_PLAN_SHARING.md) now reduces retained payload for
repeated equal plans without merging operations. Original timing evidence remains
pinned to its pre-sharing source; the follow-up's wall-time qualification is separate.
