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
| Oil brush | Loaded bristle mark that gradually fades and narrows along the stroke |
| Crayon | Wax on paper ridges with clustered flakes and smaller dry gaps |
| Marker | Dark, broad felt nib with softened corners |
| Natural pencil | Lighter graphite with fine paper grain |
| Watercolor brush | Continuous translucent wash with smoothly varying pigment and soft edges |

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

## Pigment and paper follow-up

Oil now starts fully loaded for two brush widths, then loses load over the next
18 widths. Its radius narrows from full size toward 30% while coverage fades to
zero. Distance is measured from the gesture's start, including offscreen segments,
so clipping does not reload the brush. Earlier coverage stays fixed while the
preview grows; retracing the same gesture does not darken it. Watercolor retains
a continuous translucent wash with smooth pigment variation and a soft radial
falloff. Both multiply the captured opacity once.

Crayon leaves clustered paper gaps across roughly one eighth to one half of the
sampled stroke interior, plus smaller pits around the dry flakes. The coverage
remains anchored to document coordinates. It is visibly distinct from the fine,
lighter grain of natural pencil. Marker remains fully loaded at 100% opacity.
Oil does not mix existing layer pigment. These are deterministic PaintCraft
profiles, without a native Paint pixel-equivalence claim.

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

Before/after contact sheets at 100%, 50% and 25% opacity, on white and transparent
backgrounds, were rendered directly from the shared coverage kernels under
`docs/validation/brush-rework-2026-10-24/`. Regression checks cover all nine media's
opacity multiplication, identical retracing, oil taper, watercolor continuity, clipping,
preview-prefix preservation and one-gesture undo semantics.

The latest inspected contact sheet is under `docs/validation/brush-rework-2026-10-24/tapered/`, including all opacity columns and transparent pixels.
