# Brush media

The Classic gallery enables all nine media, plus a square brush. Studio exposes
the same choices in Brush medium. Desktop, browser and headless batches use the
same coverage and transaction implementation.

| Medium | Stroke |
|---|---|
| Brush | Existing antialiased round brush |
| Calligraphy brush | Broad diagonal elliptical nib |
| Calligraphy pen | Opposite diagonal nib |
| Airbrush | Soft radial falloff |
| Oil brush | Bristle bands |
| Crayon | Coarse grain with paper gaps |
| Marker | Translucent rectangular nib |
| Natural pencil | Fine, lighter grain with paper gaps |
| Watercolor brush | Translucent, granulated coverage |

These are PaintCraft media, not reference-qualified simulations of Microsoft
Paint's material engine. Oil does not mix existing pigment, watercolor does not
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
