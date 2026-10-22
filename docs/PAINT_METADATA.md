# Bounded paint-plan metadata

Before retaining operation plans across a compound edit, their metadata must be
bounded independently of decoded pixels. A thousand overlapping writes can touch
one tile while requiring a thousand replay indices; staged-pixel admission alone
cannot constrain that growth.

`PaintPlan` now defaults to a provisional 64 MiB per-plan metadata allowance.
`PaintPlan::with_metadata_limit` supplies an explicit allowance. An `Admission`
can set `with_paint_metadata_limit`; its `paint_plan()` factory carries that policy
into descriptor construction. All editor paint families use the factory. Replay
uses the smaller of the plan's own limit and the applying admission's limit.
Cloned admissions keep the policy; descriptor/index storage remains plan-local.

## Accounted storage

The declared total includes:

- Retained descriptor vector capacity times its element size.
- Tile-index usable capacity times `(TileCoord, Vec<usize>)` payload size.
- Every tile's write-index vector capacity times `size_of::<usize>()`.

Growth is fallible, uses checked byte arithmetic and validates returned capacities.
Descriptor and per-tile vectors retain amortized growth, clamped to remaining
allowance. The tile map grows only for a new coordinate when it has no spare
capacity. Repeated coordinates reuse existing buckets, and provably default/no-op
missing tiles do not require speculative table growth. Adjacent compatible writes
can still coalesce without allocating another descriptor.

These are capacity-weighted payload declarations, not allocator layout or RSS.
std HashMap does not expose bucket/control allocation size; hash controls, allocator
rounding and temporary reallocations are excluded. A growth whose returned capacity
exceeds the allowance fails the attempt; allocation may already have occurred and
is released when the failed builder/index is dropped. Fixed stack headers and input
command/span/geometry/font buffers are not included. No physical peak-memory claim
or process-wide metadata budget follows from this per-plan bound.

## Failure and publication

Descriptor exhaustion occurs while constructing a private plan. Tile/index
exhaustion occurs before the operation's tile preparation or pixel replay. Both
return `resource.limit` with a metadata diagnostic and no decoded allocation denial.
The latter distinction is intentional: releasing decoded history cannot change a
fixed metadata policy, so native decoded-capacity retry must not wait on it.

Earlier operations in a compound edit may already have prepared private tiles.
Session failure discards that prefix and preserves owner pixels, ticket, selection,
history and save acknowledgement. The failing operation's index remains attached
to the error. This does not front-load the complete batch or turn low-level raster
builders into a standalone transaction API. Callers must abandon failed private
attempts and retain their existing atomic publication boundary.

## Executed scope and remaining work

Raster tests cover descriptor caps/coalescing, repeated indices with one touched
tile, aggregate table/vector exhaustion before decoded materialization, exact replay
under sufficient capacity, and bucket/default reuse at a tight boundary. Editor
tests inject the policy into eight painting families and fail a later ellipse after
a successful private prefix, checking full lease release and unchanged owner/history.
[The validation record](validation/paint-metadata-2026-10-21/validation.json) pins
source, commands and logs.

The 64 MiB default is an explicit safety baseline, not a benchmark-selected optimum.
No throughput or RSS gain is claimed; overhead remains unqualified. Historical paint/native timing
records remain frozen to their measured source and do not qualify this guard's
cost. Whole-batch planning/reservation, input/geometry/command limits, coordinated
all-service metadata admission, physical pressure and native latency remain open.

A future compiled compound batch must share an aggregate allowance across retained
plans, rather than multiplying this per-plan limit by its operation count. It also
needs correct planning boundaries for data-dependent floods and shared-layer copies.
The current executor still builds and releases one operation plan at a time.


[Read-only paint projections](PAINT_PROJECTION.md) now provide exact logical
pixels for data-dependent preparation without decoded replay. Ordinary painting
shares the bounded index builder, and flood accepts fallible immutable sources.
Aggregate program ownership and upfront whole-batch reservation remain open.


[Combined logical programs](PAINT_PROGRAM.md) now append ordered plans under one
capacity-accounted tile index, including incoming descriptor storage during
transfer. A read reaches its preceding source once. This is a per-program bound;
complete batch/source-lineage integration and shared service admission remain open.


Paint descriptors and indices now have [shared ownership-based admission](SHARED_PAINT_METADATA.md)
in addition to their fixed per-plan/program policy. Plan clones share charged
immutable storage; mutation copies under admission. Metadata denials carry their
own class and never enter decoded-history reclamation. Full-attempt metadata
waiting and aggregate geometry/service/physical budgets remain open.
