# Read-only paint projection for compound preparation

Whole-batch planning needs to classify data-dependent fills against earlier
planned pixels without replaying decoded tiles. `PaintProjection` provides that
raster prerequisite. It owns ordered descriptors and a bounded layer-local read
index; it owns no source raster, pixels, tile versions or decoded leases. The
preceding immutable source is supplied for each read, so callers can compose
logical views without making GPU/UI state authoritative.

## Construction and reads

`PaintPlan::into_projection_checked` consumes descriptors and builds the same
capacity-accounted tile/write index now shared with ordinary paint replay. Bounds
must describe a zero-origin layer with u32 dimensions; empty bounds are allowed.
The original per-plan metadata allowance covers descriptor vector capacity, usable
map payload and all write-index vector capacities. Growth is fallible and checks
returned capacities. Fixed quota failures carry no decoded allocation denial.
`metadata_bytes` exposes that declared payload for later aggregate ownership;
`tile_coords` exposes touched coordinates without collecting another list.

A projection is source-independent: transparent/default replacement writes cannot
be discarded based on an assumed base. Ordinary raster replay retains its existing
missing/default proofs, private/weak-only COW handling, staged-pixel checks and
operation reservation. This distinction means a huge default write may have a
cheap ordinary replay while its source-independent index exceeds its fixed cap.
No general optimization or threshold choice follows.

`read_checked` reads its preceding source once and evaluates indexed writes in
original order, applying only descriptors covering the exact requested pixel.
Straight-alpha/source-over rounding, hidden RGB and 16-sample replacement coverage
use the existing numerical policy. Outside bounds is transparent without reading
the source. Entry/completion and every 1024 visited descriptors check cancellation,
including long lists whose rectangles do not cover the requested pixel. Source
errors and cancellation return no partial pixel. Source lifetime/identity and
aggregate chain work remain the owning planner's responsibility.

`into_plan` releases the read index and recovers the original descriptors for
admitted ordinary replay, without recomputing geometry. Projection bounds constrain
the read index, not those original areas: the owner must preserve captured clipping
and source identity when replaying, especially across layer expansion. No
projection is publishable document state.

## Fallible flood source

The existing scanline classifier now delegates to `region_from_source_checked`,
whose immutable source can return an error. Raster callers supply their existing
exact getter; projection callers can compose planned views. Seed/clip/tolerance,
bitmap/frontier/output accounting, scan checks and numerical behavior remain the
same. A source error drops the private region, and a seed outside the clip does
not read the source. Scratch admission still precedes seed sampling.

## Evidence and remaining work

Six new tests check ordered chained reads against scalar source-over and independent
replacement coverage, hidden RGB and tile boundaries, unchanged decoded usage and
weak source versions, recovered admitted replay, fixed index quotas and signed/
invalid bounds. Construction and long noncovering reads cancel at every observed
checkpoint; source failure propagates through reads and flood classification.
Projected flood matches an independent BFS for all 512 binary 3x3 inputs, nine
seeds and four tolerances (18,432 cases) after two earlier planned writes.

[Validation](validation/paint-projection-2026-10-21/validation.json) pins source,
commands and compressed logs. This is a correctness/capability baseline. The [preparation comparison](../benchmarks/results/paint-projection-2026-10-21/DECISION.md)
retains 160 statistical samples and 540 process observations, tiny gains and
severe chained/no-op regressions. Sequential editor replay remains. Native
cancellation/input/display tails and aggregate physical budgets remain unqualified;
prior performance evidence stays frozen to its own source.

The editor still executes its batch sequentially. Retaining a bounded complete
program, layer duplication/deletion/expansion lineage, aggregate metadata and
scratch leases, conservative/exact decoded peak planning, effects and upfront
whole-batch reservation/publication integration remain required. A read projection
alone does not satisfy those gates. No dependency or wire/file policy changes.


[Combined logical programs](PAINT_PROGRAM.md) now append ordered plans under one
capacity-accounted tile index, including incoming descriptor storage during
transfer. A read reaches its preceding source once. This is a per-program bound;
complete batch/source-lineage integration and shared service admission remain open.
