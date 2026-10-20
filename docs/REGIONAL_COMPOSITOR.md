# Exact regional CPU composition and bounded reuse

`paintcraft-render::render_region` returns straight RGBA8 at native resolution for
one nonempty canvas-space rectangle. Malformed/outside rectangles fail before
subtraction or allocation. The existing render admission ceiling applies to the
requested output, rather than the entire sparse canvas. A 3×2 rectangle of a
1,048,576×1,048,576 canvas therefore needs only 24 output bytes. Full export still
renders its complete requested area. The borrowed-span integer kernel and finite
layer clipping are unchanged.

## Cache contract

`CompositeCache` aligns output tiles to a caller-selected 1..1024 pixel grid.
Storage tiles remain 256×256. A partial output rectangle can require an entire
intersecting composite tile; it never materializes the rest of the canvas.
There is one current full composite and one lower-layer prefix per coordinate
and domain, subject to eviction. Prefixes stop immediately below the uppermost
relevant visible layer. Resuming composition preserves every RGBA8 rounding
boundary and the original bottom-to-top order. Intermediate layers are not
regrouped, and arbitrary prefixes/groups are not supported.

Keys describe the document identity, output rectangle and ordered relevant layers:
layer identity, offset, dimensions, opacity, implicit default RGBA, resident tile
coordinates and immutable source versions. Visibility/clipping determine inclusion.
The current policy is exclusively flat encoded-sRGB RGBA8 default-v1; there are
no group, profile, effect or mask semantics in this document model. Introducing
any of these requires extending the dependency plan before caching them.

A publication epoch/state UUID is deliberately not the source pixel key: unchanged
regions survive atomic publication and undo/redo. Existing desktop freshness guards
still decide whether a completed frame may be presented. Selection/tool overlays
are not cached. Raster previews are composed from their private staged document.

`RasterLayer::tile_versions` uses weak Arc identities. Keys neither hash pixels
nor retain strong source tile snapshots. Weak control blocks prevent address reuse
from masquerading as the same tile. COW writes and even unshared `Arc::make_mut`
writes detach the identity. Weak keys can keep control blocks alive, but not the
decoded tile Vec. This does not replace the still-missing logical tile store and
resident lease authority.

Committed and preview partitions have independent byte and entry limits. Preview
lookups may reuse committed results, but preview misses never evict committed
entries. Clearing previews releases their cached pixel Vecs and version keys;
the preallocated entry table stays allocated and charged. A cancelled render may
warm complete tiles/prefixes, but never publishes a partial tile or returns a
partial frame. Caller-owned output buffers cannot pin cache entries.

## Accounting and observable work

Budgets include cached pixel Vec capacities, entry table/key Vec capacities, and
a conservative 128-byte charge per weak identity, including duplicates. This is
an admission weight, not allocator overhead, actual source residency or process
RSS. Allocation failures return errors or bypass caching when the key cannot fit.
The returned dense region, current tile scratch and source/history ownership are
outside this persistent cache budget. Every caller still owns its output Vec.

`RenderStats` reports full cache hits, lower-prefix hits, recomposed output tile
count/pixels and requested output pixels. A single-pixel change currently replaces
a source storage tile, so it invalidates 65,536 output pixels at all measured
64/128/256 composite granularities. The smaller grids do not yet provide finer
stroke-level invalidation. There is no mipmap/GPU texture validity here.

## Application integration

The desktop CPU fallback and deterministic CPU harness share a bounded derived
cache across frames and cancelled jobs. Native execution has one admitted render
worker. Only that cache is locked; immutable document reads and expensive editing
never require a document-global lock. Existing document/epoch/gesture acceptance
checks remain authoritative for presentation. GPU-first rendering and device error
fallback remain in place; no CPU/GPU performance dispatch claim is made.

The CPU cache uses 256-pixel output tiles, a 16 MiB committed budget, a 4 MiB preview
budget and at most 512 entries per domain. Single relevant layers retain uncached
composition because the measured sparse case did not benefit. Outputs smaller
than one storage tile also bypass caching conservatively; a tiny-image crossover
is not qualified. Cache construction failure retains ordinary CPU composition.
A poisoned derived-cache mutex retires all entries and resumes uncached CPU
composition; an injected worker-panic test verifies subsequent frames recover.
Native job admission includes 20 MiB for persistent caches and up to 32 MiB
for simultaneous full/prefix key construction, in addition to its existing
output/staging allowances. A unified process-wide residency authority remains
future work.

Desktop frames still allocate a complete dense canvas result, convert it to egui
pixels and upload the whole texture. Cached regions reduce composition work; they
do not yet eliminate full-frame assembly or implement viewport-only presentation.

## Verification and measurement

Regional tests cover partial edge tiles, negative offsets, opacity, checked ranges,
cancellation and partition reassembly, plus a hand-calculated translucent pixel.
Cache tests exercise 64/128/256 grids, exact localized edits, prefix reuse, repeated
hits, undo/redo, opacity/visibility/reordering, centered resize, layer deletion,
cross-document identity, partial viewports, tiny budgets, eviction and cancellation.
Weak-version tests verify COW sharing and decoded-source release. Desktop tests
cover actual CPU preview/cancel/commit/undo/redo and retain stale-worker guards.
The independent scalar/all-alpha tests and canonical document fixtures still apply.

The replay benchmark uses admitted atomic pencil/property commands, recorded
history and identical required output for every contender. Divan times each
edit/render/output destruction after a first render. Process measurements also
include fixture construction, first render, history and teardown. Local edits
use 1/5/20/100-layer sparse and five-layer dense fixtures. A global opacity change
and a cropped viewport are separate controls. History has the same 32 MiB limit
for all contenders. This is not an input-to-display latency measurement.

The [cache comparison evidence](../benchmarks/results/cpu-cache-2026-10-20/DECISION.md)
records selection, regressions and reproducible raw timing/resource data. The
[earlier row-span measurements](CPU_COMPOSITOR.md) remain a separate comparison.

## Remaining scope

Finer operation dirty rectangles, effects' input/output halos, arbitrary layer
prefixes/groups, parallel tiles/bands, single-surface borrowing, mipmaps, persistent
GPU residency and viewport texture updates remain incomplete. Preview path replay,
history compression/spill and broader interoperability still need their own
correctness and resource/performance qualification.
