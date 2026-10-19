# Brush rendering baseline

The desktop round brush uses `raster.stroke_path` with `brush: "smooth-round"`
in batch v2. Existing `round` and `square` operations retain their integer stamp
semantics; batch v1 and its canonical conformance fixtures are unchanged. Pencil
and square brushes remain crisp. This is a provisional quality baseline, with
no claim of parity with a captured Windows Paint build or measured performance.

## Coverage contract

Points identify pixel centres `(x + 0.5, y + 0.5)`, for odd and even sizes.
Consecutive centres form straight segments swept by a disk of radius `size / 2`.
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
