# Bounded CPU composition partitions

`paintcraft-render::parallel::render_in_pool` composes one checked native-resolution
rectangle on an explicitly supplied Rayon pool. The caller owns thread count,
resource admission and cancellation. Rendering creates no pool, uses no global
pool and does not install an independent nested executor. Reentrant calls on the
same pool reuse its workers. The output retains the ordinary regional pixel limit.

## Two exact implementations

Row bands borrow contiguous groups of complete output rows. Each job writes its
own slice directly, using the existing root-order compositor. A final short band
has exactly its requested rows. Band height is checked in 1..1024; chunk arithmetic
is bounded by actual output height, including on 32-bit targets.

Tile jobs borrow individual disjoint row fragments into private tile views. The
views contain mutable slices into the single output Vec, not copied pixel buffers.
Safe Rust moves every fragment into exactly one job; there is no unsafe strided
aliasing, output lock or second full-frame allocation. Partial right/bottom tiles
retain exact extents. Jobs run the same shared row kernel in original layer order.

Both preserve transparent-black initialization, integer default-v1 source-over,
opacity rounding, offsets, finite clipping and straight-alpha hidden-RGB behavior.
Sources remain immutable sparse COW rasters. No document pixels/history are mutated
and no document-global lock is held. Partitions are valid for the implemented
flat pointwise policy only: effects with halos, groups, profiles and masks would
need dependency-aware planning before using these algorithms.

## Admission and cancellation

The tile view budget includes entry Vec capacity and every row-reference Vec
capacity. A checked required-capacity estimate rejects oversized requests before allocating
the pixel output; actual capacities are checked during construction. Pixel output,
Rayon scheduling metadata, source/history ownership and thread stacks are separate
from this view budget. Allocation failures return `resource.limit`. Bands need no
tile-view allocation. No claim of unified process-wide residency admission is made.

Cooperative cancellation is checked before allocation, during view construction,
between relevant layers/rows and after all jobs finish. Workers own disjoint writes;
Rayon joins the active jobs before the private result is dropped on error. Errors
never return a partial image. There is no priority scheduler or GPU work in this
API. Cancellation-age and interactive interference qualification remain pending.

## Shipping consumer

`paintcraft-cli render --render-workers N` accepts an explicit 1..64 worker count.
The default is one and retains serial rendering. For N > 1 the CLI constructs one
bounded pool, renders through 64-row bands, then encodes/writes the complete result.
The pool size is the requested render-worker budget; codecs run afterwards on the
caller, without an additional Rayon codec pool. This is not an automatic host-wide
thread selection policy. Worker stacks/resource costs remain the caller's concern.

`paintcraft-formats::export::export_flat_with` accepts an exact renderer callback,
checks loss codes before invoking it, and validates complete output length before
encoding. It preserves named loss acknowledgements and existing atomic destination
publication. The regular export API delegates to the same checks with serial
rendering. Pool/render failures happen before destination bytes are published.

Desktop cache/GPU dispatch is unchanged. A full parallel frame is not assumed to
beat the existing local-edit cache, and no second native foreground pool is added.
Integrating partitions into admitted cache misses and interactive/background
scheduling remains work; the shared API is independent of GUI state.

## Correctness and performance evidence

Tests cover 1/2/4 workers, exact full/cropped regions, opacity/visibility/clipping,
partial edge tiles, unchanged source versions, invalid partition sizes and view
budgets, cancellation during active work, checked huge sparse regions and
same-pool nested execution. CLI tests export real layered PCND files at 1/2/4
workers, independently decode PNG, compare exact pixels and a hand-calculated
translucent pixel, and reject worker counts 0/65 without writing. Export callback
tests reject unknown loss codes before work and malformed buffers before encoding.
Existing independent all-alpha arithmetic, canonical PCND workflows and real Metal
parity remain applicable to the shared row primitive.

BF06 compares serial, 1/2/4-worker 64-row bands, four-worker 256-row bands and
four-worker 64/256-pixel tiles. Fixed fixtures cover tiny, opaque, sparse 5/20/100
layers, dense mixed-alpha layers and entirely clipped layers. Every timed process
checks output length and five independently evaluated point probes. Whole-image
frozen scalar preflight is a separate correctness gate. Results are consumed and
destroyed; first render and pool setup are included in process time.

Criterion 0.8.2 is development-only and exports raw 100-sample JSON/CSV, confidence
estimates and outliers. Default optional Rayon analysis and plotting are disabled.
This capability was checked against Divan 0.1.21, whose public CLI provides a table
but no raw internal-sample export. Divan allocation targets remain useful, but their
previous summaries do not close the raw-sample requirement. Older span/cache
in-process evidence needs a raw-sample follow-up; its process evidence is retained.
See [Criterion's source/tests](https://docs.rs/crate/criterion/0.8.2/source/tests/criterion_tests.rs)
and [CSV output documentation](https://bheisler.github.io/criterion.rs/book/user_guide/csv_output.html).

The [local decision and raw evidence](../benchmarks/results/cpu-parallel-2026-10-20/DECISION.md)
record process time, steady render latency, memory, CPU cost, regressions and
retained calibration. No cross-host, GPU, input-to-display p99 or scheduler priority
claim is made. This slice leaves T12/T13 and BF06 partial under the full roadmap.
