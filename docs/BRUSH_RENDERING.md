# Brush rendering baseline

The desktop round brush uses `raster.stroke_path` with `brush: "smooth-round"`
in batch v2. Existing `round` and `square` operations retain their integer stamp
semantics; batch v1 and its canonical conformance fixtures are unchanged. Pencil
and square brushes remain crisp. This is a provisional quality baseline, with
no claim of parity with a captured Windows Paint build or measured performance.

## Coverage contract

Integer points identify pixel centres `(x + 0.5, y + 0.5)`, for odd and even sizes.
The additive v2 `point_scale: 256` option records exact document positions in
1/256-pixel units for `smooth-round` and the [textured brush media](BRUSH_MEDIA.md), without a center offset. Omitted
`point_scale` and an explicit value of 1 retain historical integer semantics.
Desktop round brush input preserves subpixel movement at every canvas zoom and
display scale. Pencil, square brushes, eraser, shapes and selections retain
integer pixel input. Invalid scales and out-of-range fixed-point coordinates
are rejected before publication.
Consecutive positions form straight segments swept by a disk of radius `size / 2`.
A one-point gesture is a disk. Caps and joins are round. No spline is applied.
Every pixel has 16 sample positions on a regular 4×4 grid, at offsets
`(i + 0.5) / 4` in each axis. Samples inside any segment capsule are covered.
Retracing and shared joints within an operation do not deposit more paint.

The rasterizer intersects capsule scanlines using f64 arithmetic after integer
rebasing to the clip origin. It unions subpixel intervals before resolving
disjoint coverage runs. This is supersampled coverage, not a blur filter.
An independent point-to-segment distance oracle tests the scanline implementation.
Cross-platform floating-point boundary qualification remains pending.

For `source-over`, source alpha becomes `(alpha * samples + 8) / 16` using integer
division; the normal straight-RGBA8 source-over kernel then blends once. For
`replace`, covered samples select source channels, uncovered samples retain
destination channels: `(src * samples + dst * (16 - samples) + 8) / 16` for each
stored RGBA channel. Full coverage exactly replaces. Canvas, target extent and
rectangular selection clip the result before editing.

The existing aggregate batch stroke limits apply. Smooth rasterization also
allows at most 1,048,576 generated subpixel scan rows per operation, including
overlapping segments. These are provisional admission limits. Long desktop
gestures still split at the batch operation point limit; overlapping chunk
joints can deposit twice for translucent colours, as with integer brushes.

## Brush opacity and explicit paint mode

The shared Brush options include integer opacity from 0 to 100 percent, initially
100. It is captured at pointer-down with the color, size, tip, button and layer.
The effective source alpha is `(color_alpha * min(percent, 100) + 50) / 100`.
Coverage then scales this alpha, and layer opacity remains a separate compositor
input. Changing the preference adds no document history. Each document tab retains
its own tool options.

Preview and release share one captured `Stroke`, including an explicit mode.
Brush and Pencil use source-over; zero-opacity Brush and alpha-zero Pencil strokes
leave pixels, StateId, dirty state and history unchanged. Eraser uses replace
explicitly, with its replacement color captured at pointer-down. Classic restores
the first opaque raster ground in document root order, falling back to white for
imported or transparent documents; Studio replaces pixels with transparency. Alpha zero alone no longer selects erasing behavior. CLI operations
retain their explicit `mode` contract.

Within a single operation the swept geometry is unioned before blending, so
retracing cannot exceed the selected opacity. The previously documented long-path
chunk-boundary limitation still applies. Tests cover captured secondary color,
partial antialias coverage, preview/release identity, no-op gestures, cancellation,
undo/redo and native-document round trips.

## Preview and publication

Path previews and released strokes use the same command builder, private staged
execution, and document compositor. The preview includes layer ordering, opacity,
selection clipping and erasing; it never mutates the published document or history.
Release publishes one transaction, and cancellation discards the unpublished path.

Native previews use the existing bounded render worker and the CPU/wgpu compositor
selection. A separate preview revision rejects stale worker results, including
cancellation and a new gesture on the same document. Preview jobs reserve an extra
256 MiB of provisional scratch admission. Worker results may lag input; a pending
result never replaces a newer presentation. Headless GUI tests run the same staging
and composition synchronously and compare the displayed preview to release bytes.

The canvas retains nearest-neighbour display for zoomed pixel inspection. Antialias
coverage is stored in document pixels, so it persists on release and image export.

## Preview scheduling and development builds

Pointer movement now coalesces behind one in-flight render instead of cancelling
it on every event. An intermediate frame may publish only within the same live
gesture and document publication. Cancelled/restarted gestures and committed
changes retire those frames. The worker then renders the latest captured samples.
A barrier-based test checks progress and final pixel equality.

The default desktop dev profile optimizes core pixel arithmetic, raster, document,
editor and compositor hot loops at level 2, retaining debug information, overflow
checks and the desktop controller's ordinary debug build. A manual CPU-only test
is available with `cargo test -p paintcraft --lib profile_round_brush_preview -- --ignored --nocapture`.
On this development machine, one 1000×486 / 32 px / 1000-sample run changed from
133.24 ms staging + 90.93 ms composition to 10.64 ms + 2.56 ms after the profile
change. At 4000 samples the corresponding result was 488.92 + 89.71 ms versus
35.10 + 2.40 ms. These are local diagnostic samples, not full-window frame latency
or the roadmap's statistical benchmark qualification. Native GPU rendering still
uses the existing readback path; incremental tile uploads remain future work.

## Gallery media and preview scheduling follow-up

All nine brush media are now available in Classic, Studio, web and CLI batches;
see [coverage, limits and qualification](BRUSH_MEDIA.md). Growing shape previews
now retain in-flight progress within the same owner/style/view, with exact latest
frame convergence and stale-owner rejection. See [shape scheduling](SHAPE_PREVIEWS.md).
