# Brush media

The Classic gallery enables all nine media, plus a square brush. Studio exposes
the same choices in Brush medium. Desktop, browser and headless batches use the
same coverage and transaction implementation.

| Medium | Stroke |
|---|---|
| Brush | Existing antialiased round brush |
| Calligraphy brush | Broad diagonal nib with a pixel-wide antialiased edge |
| Calligraphy pen | Finer opposing diagonal nib with the same edge treatment |
| Airbrush | Discrete spray particles with radial density |
| Oil brush | Continuous loaded pigment with bristle bands and a dry exposed edge |
| Crayon | Dense wax coverage with sparse paper pits |
| Marker | Dark, broad felt nib with softened corners |
| Natural pencil | Lighter graphite grain with more paper gaps than crayon |
| Watercolor brush | Continuous translucent wash with granulation and edge pooling |

These are PaintCraft media, not reference-qualified simulations of Microsoft
Paint's material engine. The supplied interface screenshots establish the gallery
and controls, but do not provide a measured per-pixel stroke oracle. Oil does not
mix existing pigment, watercolor does not
simulate wet paper, and airbrush does not accumulate while the pointer is idle.

Each gesture captures its medium, color, opacity, size and fixed-point positions.
Overlapping dabs union their coverage before one source-over application, so
retracing a translucent stroke does not repeatedly darken that gesture. Separate
strokes build color. Grain is anchored in document coordinates. Selection clipping
keeps the original stroke's sampling phase and does not reshape the nib.

The v2 `raster.stroke_path` batch command accepts `calligraphy-brush`,
`calligraphy-pen`, `airbrush`, `oil`, `crayon`, `marker`, `natural-pencil` and
`watercolor` in `brush`, alongside `round`, `square` and `smooth-round`. Textured
media use source-over. `point_scale` is 1 or 256, with the existing integer or
fixed-point center convention. CLI `apply` and `render` support every medium.

Temporary coverage is bounded to 4,194,304 pixels, 67,108,864 dab-pixel visits and
262,144 output spans. These explicit scratch/work policies are separate from
decoded tile residency. Over-limit work fails before publication. Geometry and row
work check cancellation; tile preparation, selection and undo remain shared with
other paint operations. Large or very long textured strokes can reach these
provisional limits; incremental textured preview caching remains future work.

Validation covers distinct repeatable media, subpixel preview/release identity,
clipping without changed interior coverage, retracing, opposite nib directions,
transparent hidden RGB, opacity, cancellation, limits, undo/redo, actual CLI
project/export round trips and browser preview-to-publication pixel identity.
The frozen sequential benchmark executor rejects the newly added media and
retains the original algorithms for existing benchmark fixtures; its manifest
records this compatibility-only change.

The published CLI schema also enforces each medium's blend and fixed-point rules.
An independent JSON Schema validator and actual CLI apply processes agreed on all
330 combinations of brush, point scale, mode and coordinate boundary in the local
qualification matrix. The raw matrix is under
`docs/validation/editor-finishing-2026-10-24/brush-schema-matrix.json` (ignored).

The latest [Classic follow-up](CLASSIC_FOLLOWUP.md#brush-comparison) records the
real Paint examples inspected and the limits of that visual comparison.

## Continuous pigment follow-up

Oil and watercolor no longer expire after six or eight brush widths. Movement
continues to deposit their loaded pigment along the whole gesture. Their exposed
tip edges feather into the canvas while interior coverage remains continuous.
This is a geometry-based edge treatment, not sampling or mixing existing layer
pixels. Document color and selection coverage still enter through the ordinary
prepared paint transaction. Both media multiply the captured brush opacity once,
just like every other medium.

Marker interior coverage is 247/255 at full source opacity, compared with the
previous 115/255. Its felt tip has softened corners. Crayon leaves about 0.8% of
interior paper samples uncovered, with dense wax elsewhere. Natural pencil leaves
about 23% uncovered and has a lighter graphite deposit. These are deterministic
profile parameters, not benchmark measurements or native Paint equivalence claims.
Airbrush remains discrete, with its highest particle density toward the center.
The broad and fine opposing calligraphy nibs resolve their edge distance in canvas
pixels so the narrow nib does not collapse its antialiasing fringe.

All textured media still recompute bounded full-gesture coverage for a preview.
The [incremental smooth-path cache](INCREMENTAL_COVERAGE.md) only serves the ordinary
round brush. This rework does not claim to remove that textured-preview cost.
The existing scratch, work, output-span, residency and publication policies remain.

Reproduce the actual raster comparison without opening a window:

```sh
cargo run -p paintcraft-formats --example brush_contact_sheet -- \
  docs/validation/brush-media-2026-10-24/contact-sheet
```

The example writes white and transparent PNGs plus a checkerboard presentation,
raw straight RGBA buffers, and SHA256 hashes in `recipe.txt`. Each row is one of
all nine media. Each opacity column uses 100%, 50% or 25% source alpha and shows
a 510-pixel straight stroke, a curve, and a retraced straight stroke at size 18.
It invokes the same raster coverage and integer source-over math used by the
editor; the images are implementation evidence, not screenshots of Microsoft Paint.
The output remains ignored validation data.
