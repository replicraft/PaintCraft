# Incremental smooth-path sample coverage

The desktop renderer can retain one growing smooth path's sample union. It scans
only appended capsules and unions their subpixel intervals into retained scan rows.
Coverage still resolves against the whole union and is applied once against the
current private source raster. It never paints a translucent suffix over the
previously blended preview. Stroke policy, hidden RGBA, sample centers and exact
16-sample replacement math remain unchanged. Final Session execution does not
require the cache and continues to record one transaction per changed stroke.

## Identity and failure

Reuse requires identical size, point scale and absolute clip, plus an exact point
prefix. Truncation, interior changes and changed geometry policy rebuild from the
full current path. Color, document identity and layer ID are deliberately absent:
the cache contains geometry alone; current pixels, blend mode, layer origin and
selection determine the paint operation separately. The editor supplies its current
canvas/layer/selection intersection as the clip. Distinct strokes and compound
previews execute in order; the single entry may churn when several paths alternate.

An initial single-point disk is contained in the first capsule. Its retained samples
can remain, but its scan-row charge is removed when the first segment replaces it.
Cumulative scan-row accounting therefore retains the full-path work limit. Cached
unions are normalized after resolving; retraces neither duplicate retained intervals
nor accumulate opacity. Every update still validates all input points and compares
the complete prefix. This does not remove command parsing, source staging, coverage
resolution, compositing, texture transfer or display work.

Cancellation, invalid input and geometry errors retire the cache's partial state.
Cancellation also reaches the desktop render worker's checked execution entry point,
including paint-plan indexing/preparation/replay. Private partial pixels are discarded;
late frames remain subject to existing document/preview/device guards. Poisoned
geometry locks clear derived data before reuse. Returning to committed rendering
clears the geometry entry; a blocked render request can retain it until admitted.

## Resource scope

The provisional default retained metadata charge is 8 MiB: actual point/run Vec
capacities, State size and 128 bytes per retained row for tree/node allowance. An
update exceeding it returns its correct coverage and drops retained state; zero
limit disables retention. This is an explicit accounting rule, not a physical
allocator/RSS bound. Temporary scan rows, result spans, parsing and raster tiles
retain their separate policies. Oversized/cancelled work never gains a higher
scan-row allowance from reuse. The native declared rendering allowance includes
this additional 8 MiB alongside existing preview/composite allowances.

One admitted render worker serializes access. The UI does not lock geometry while
that worker runs. CPU and GPU display both consume the same staged preview; only
the CPU display path is exercised by the current process comparison.

## Verification

Raster tests compare every prefix across size/scale/clip/translation changes,
retraces and duplicate points against full coverage, whose sample-distance oracle
is independent of scanline generation. Tests cancel each observed extension check,
verify retirement/rebuild, inject zero retention and compare work-limit outcomes
after single-point extension. A separate check-count regression ensures extending
a long cached path does not rescan its old segments. Editor tests compare current
selection/origin/source-over/replacement pixels and preserve the source document.
Desktop tests cover the actual staged/composited frames, cancellation, committed
cache retirement and panic recovery. Existing preview/publication/undo tests remain.

Performance evidence lives in
[the desktop replay decision](../benchmarks/results/preview-geometry-2026-10-21/DECISION.md).
It measures batch construction, private staging, CPU composition and teardown in an
optimized desktop test executable. It does not establish native event/display tails,
GPU upload costs, scheduling priority, fairness, long-session RSS or Windows parity.
