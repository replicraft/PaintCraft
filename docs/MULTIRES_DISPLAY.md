# Bounded composite-before-filter display

The CPU display baseline now renders zoomed-out views without materializing their
entire native-resolution source ROI. This implements the active flat RGBA8 portion
of RENDER-01..07 and the compositor owner's composite-before-resample rule. It is a
provisional display filter, not qualified Windows Paint/Paint.NET sampling parity.
Native-resolution display and export numerical semantics remain unchanged.

## Request, grid and numerical policy

`paintcraft_render::display::DisplayRequest` describes a nonempty in-canvas source
region and an integer level 0..20. A level samples `2^level` document pixels along
each axis. Source bounds align outward to that global grid and clip to the finite
canvas. Output dimensions are ceiling divisions by the sample step. Panning preserves
the same grid rather than moving sample boundaries with the viewport.

The native-resolution baseline first composites every source pixel with ordered
`paintcraft.core/default-v1` layer-opacity/source-over rounding. For a box containing
N actual canvas pixels, accumulate A = sum(alpha) and C = sum(channel * alpha).
Round the output alpha as floor((A + floor(N/2)) / N). If that rounds to zero,
output transparent black. Otherwise each straight output channel is
floor((C + floor(A/2)) / A). Sums use checked-admitted u64 ranges; no source channel
is changed. This averages encoded-sRGB code values under alpha weights, not linear
light. It does not separately downsample layers or first quantize egui premultiplied
pixels. Ordinary ColorImage premultiplication follows the final straight result.
A GPU scaled compositor must mirror this composition/filter/conversion order.

Canvas-edge boxes include only actual canvas pixels in N. Layer clipping inside
the canvas still contributes transparent pixels normally. A final partial box keeps
its full texel footprint on screen, clipped at the canvas edge: texture coordinates
are not stretched across shortened edge boxes. The frontend selects the largest
power-of-two step no larger than one physical screen pixel. At zoom >= 1, level zero
preserves accurate native pixels; hardware limits do not silently soften them.

## Bounded execution and content reuse

The reducer processes at most 32×32 output samples per default cache tile. It first
tries to prove the complete ordered composite uniform using immutable layer metadata.
`RasterLayer::uniform_region` checks installed tiles or coordinate lookups, whichever
set is smaller, and only trusts untouched 64-pixel cells' initial colors. Installed
initial colors may differ from the layer default. Writes, even uniform writes, may
conservatively prevent a proof. No pixel hash, tile lease or new authoritative raster
representation is introduced.

An unknown region recursively splits until each native composition block is at most
256×256. Its exact pixels contribute to bounded accumulators, then the final output.
Uniform subregions contribute weighted counts without scanning native pixels. This
allows an implicit million-square canvas to produce a 1024-square display, and a
changed 256-square tile to require at most that many native pixel visits. Dense input
still requires correct source processing; this is not approximate layer-first LOD.

Coarse output cache keys include source regions, ordered layer properties/versions,
and sampling level. Global source/output tile alignment preserves overlap reuse when
panning. Committed/preview partitions are independent; previews may reuse unchanged
committed entries. Complete entries may warm before cancellation; partial entries
never install or return. Large dependency keys bypass before enumerating huge region
grids. Uniform results bypass key construction altogether. Current defaults are
32-sample output tiles, 4 MiB committed / 1 MiB preview and 128 entries per partition.
These are a bounded baseline configuration, not an optimal-layout claim.

## Native integration and boundaries

Frame freshness includes source region and sampling level in addition to document,
publication, preview and device tokens. Old progress cannot cross a level change.
`App::rendered_size()` and `rendered_sample_step()` describe CPU display packing;
source-region dimensions alone are insufficient to index a coarse buffer. The painter
uses each accepted frame's global footprint and clips it to the actual canvas.
Native worker output admission uses coarse output area and an added 15 MiB allowance
for display partitions, transient keys, native block scratch and accumulators.
Declared bytes and local cache weights do not qualify all-service physical memory.

Scaled frames currently use this CPU oracle and the ordinary managed-texture upload.
The native GPU remains available for level-zero rendering; a zoomed-out CPU frame
does not disable it. [Exact GPU scaled display](GPU_SCALED_DISPLAY.md) is available
through explicit native experimentation; measured regressions reject automatic
global enablement. Tiled presentation, asynchronous transfers, physical pressure-driven eviction and
native input/display tail qualification remain open. Final exports/clipboard and
sampled-composite tools use authoritative native-resolution document semantics;
large exports retain their explicit existing limits instead of exporting display LOD.

## Verification scope

Independent full-native composition and direct box summation verify offsets, opacity,
hidden RGB, partial canvas edges, global alignment, cold/warm levels and cancellation.
A two-pixel red/blue fixture explicitly distinguishes compose-then-filter from
layerwise filtering. Preview caches preserve committed entries and distinct levels
coexist. Uniform metadata checks cover COW, installed initial colors, sparse absence,
invalid bounds and cancellation. A million-square fixture checks bounded native work
and known weighted color after a black tile edit.

Desktop checks cover level-stale previews, poisoned cache fallback, native admitted
large-document output without GPU disablement, fit/actual-size/history behavior,
forced layout passes and partial-edge mesh footprints/clipping. Native completed
managed-upload comparisons evaluate full-source materialization, bounded reduction
and cache reuse with independent pixel/source guards. These do not establish Windows
reference conformance, physical-device changes or OS event/display latency.

[The measured decision](../docs/benchmarks/multires-display-2026-10-21/DECISION.md)
retains an existing cached full-resolution control, tiny ties, dense cache tradeoffs,
the sparse-4K process improvement, raw data and the distinction from GPU scaled
composition or historical/native input-display qualification.
