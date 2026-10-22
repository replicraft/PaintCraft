# Source-bound paint lineage

`PaintSnapshot` captures an owned sparse raster and its immutable decoded leases.
`PaintOverlay` appends ordered logical paint against that source without allocating
decoded tiles. Freezing and forking retain immutable prefixes, so painting one
branch cannot rewrite a sibling or an earlier snapshot. Expansion reveals the
captured implicit default outside the old bounds; earlier out-of-bounds descriptors
cannot become visible later.

This is a preparation primitive for whole-batch compilation. The editor still
executes batches sequentially. It does not yet simulate complete tile ownership,
reserve a complete batch peak, or integrate structural commands and error attribution.

## Source proofs and reads

The original PaintProjection remains source-independent: callers provide pixels
on each read, and writes that are no-ops on one source must remain usable on another.
Its existing public contract is unchanged.

The new bound type exposes no replacement-source callback or detached replay plan.
It can therefore discard writes proven to preserve the captured implicit default
when no source or pending program tile exists. It also omits default-preserving
writes at missing source tiles while retaining ordered writes after earlier logical
changes. False negatives retain work; a source-dependent proof cannot leak into
the arbitrary-source projection API.

A wholly uniform default view needs no tile-grid walk or reader scratch. Other
readers prepare one fallible bounded vector of lineage references, then evaluate
ancestors iteratively in original order. Pixel reads allocate no new scratch.
Reads outside the captured extent return its implicit default, matching
RasterLayer::get; this differs deliberately from the free projection's outside-
bounds transparent result. Branch expansion preserves hidden RGB in that default.

Source snapshots retain input pixels as ordinary readers, rather than copying
them. A later external raster write follows normal COW and cannot change captured
pixels. Retained source leases must be included in future batch residency planning.

## Declared ownership and failure

Shared metadata accounts own source-node payload and usable source-map capacity,
ordered descriptors and indices, and reader-vector capacity. Existing fixed
metadata limits and reserved metadata factories apply. Shared ancestor payload
is owned once; metadata_bytes reports reachable lineage payload, so callers must
not add shared ancestors repeatedly across snapshots. The shared budget remains
the declaration authority.

Append consumes its branch and input on error. Cancellation or metadata denial
releases private storage without changing previously frozen ancestors. Source
capture frees owned source storage before releasing its metadata declaration.
Reader construction, ancestry lookup and reads observe cancellation checkpoints;
this does not qualify wall-clock cancellation latency.

The ownership chain has iterative teardown as well as iterative reads. Consuming
a shared Arc with into_inner avoids a losing try_unwrap result becoming the last
owner concurrently and recursing during its own drop. Long
lineages must not rely on the thread's call-stack depth to release resources.
Allocator/hash/Arc controls, transient reallocations and physical RSS remain
outside these declared payload bounds. This adds no file/wire policy or library.

## Verification and measurement scope

Regressions cover immutable branching, expansion clipping/defaults, independent
coverage/source-over pixels, external COW, huge no-op proofs, exact reserved
metadata, shared/fixed denial, cancellation checkpoints and deep small-stack
read/teardown. An independent BFS checks 18,432 binary-image/seed/tolerance cases.

The paint_lineage benchmark compares materialized, combined arbitrary-source and
bound-source preparation on the same tiny/wall/overlap/default fixtures. It checks
every pixel and canonical flood region before timing and after each process session.
Source capture and reader scratch are included in bound metadata counters; those
counters are not identical ownership scopes across modes. Retain process RSS and
setup controls separately. These are library preparation measurements, not native
whole-batch, branching-chain timing, file-I/O or input/display latency evidence.

[Source-pinned validation](validation/paint-lineage-2026-10-22/validation.json)
records 893 workspace tests (26 explicit ignores), strict all-target Clippy,
39 optimized raster tests, required Metal/desktop tests, release builds and
isolated native startup checks. The initial compile diagnostics and superseded
pre-concurrency checks remain separate from the final successful rerun.

[Matched preparation evidence](../benchmarks/results/paint-lineage-2026-10-22/DECISION.md)
retains 240 statistical samples and 780 process observations. Bound default/no-op
preparation is about 3.7–3.9x faster than combined in process measurements, but
13.5–21.1% slower than materialized. Tiny/wall overhead and overlap tier disagreement
remain. No general winner or editor-default activation follows.

Whole-batch decoded peak and cumulative allocation simulation, structural lineage
integration, joint resource reservation, aggregate geometry/font/flood budgets,
scheduling fairness, native latency and broader roadmap features remain open.
