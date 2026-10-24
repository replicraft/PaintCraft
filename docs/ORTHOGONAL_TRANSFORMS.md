# Exact orthogonal canvas transforms

The engine and CLI support `canvas.transform` in `paintcraft.batch/v2`:

```json
{"op":"canvas.transform","transform":"rotate-right"}
```

Accepted values are `rotate-right` (90° clockwise), `rotate-left` (90°
counterclockwise), `rotate-180`, `flip-horizontal` and `flip-vertical`. No other
keys or angles are accepted. V1 rejects the operation. The public typed descriptor
lives in `paintcraft_core::types::orthogonal`.

## Whole-document semantics

The operation reorients every complete layer raster, including hidden layers and
off-canvas content, around the canvas. Quarter turns swap canvas dimensions and
layer raster dimensions. Origins move with the layer extents. IDs, order, names,
visibility, opacity and default pixels remain intact. Samples copy exact straight
RGBA, including RGB at zero alpha; there is no resampling or color conversion.
Origin results outside the admitted coordinate range fail atomically.

A rectangular selection is intersected with the old canvas and its rectangle
reorients with the whole document. Empty/inactive states remain empty/inactive;
a wholly outside rectangle becomes empty. Selection contents are not an isolated
transform target for this operation. A single recorded transaction restores the
exact previous document and selection on Undo/Redo. Inverting an orientation
restores stored samples and layer extents, including off-canvas content.

These engine/CLI operations are implemented. Classic Rotate/Flip controls remain
unavailable while selection-target transform semantics and host actions are being
completed. Custom angles, resize resampling, skew and interactive handles remain
work. This tranche does not establish Windows selection behavior or pixel fidelity.

## Sparse plans and admission

`MappedPlan` generalizes the source-bound Crop plan; `CropPlan` remains a public
alias with its existing preparation methods. The same charged destination tile
map, immutable logical source and integer coordinate lineage support crop,
rotation and mirroring. Mapping composition uses checked integer signed permutation
matrices and translations. Padding outside old finite bounds remains the implicit
default when later expansion reveals it.

Preparation samples candidate non-default source tiles, retaining destination
coordinates without decoded allocation or per-pixel command descriptors. Huge
uniform rasters require no sample walk or pixel storage. A provisional cumulative
16,777,216 source-read ceiling covers crop and orthogonal transforms together in
one batch; denials retain the published owner/history unchanged. Replay can read
more samples because output tile boundaries differ from input tile boundaries.
This is a logical work limit, not a physical memory or measured latency guarantee.

Complete-batch compilation pins all earlier paint, floods, duplication and mapped
results. Later reads see those logical sources. Fresh destination tiles participate
in the same simulated private decoded peak reservation before replay. Cancellation
checks run through mapping preparation and replay, releasing partial private output.
No sequential fallback bypasses the compiler.

## Validation

- [Raster plans](../crates/foundation/paintcraft-raster/tests/orthogonal_plans.rs)
  compare all five orientations to independent dense row/column operations on
  asymmetric, one-pixel-wide/high and unaligned multi-tile rasters. They compare
  replay and logical samples, compose fifteen orientations with paint/crop/expand,
  preserve hidden RGB/default padding and verify cancellation/limit cleanup.
- [Engine tests](../crates/editing/paintcraft-editor/tests/orthogonal.rs) check
  explicit origin/selection cases, hidden/off-canvas metadata, inverse operations,
  recorded history, paint→rotate→flood→duplicate→crop→flip→paint ordering,
  full-batch pre-allocation denial, cancellation and coordinate limits.
- [CLI tests](../apps/paintcraft-cli/tests/cli/orthogonal.rs) execute every wire
  transform and reopen PCND to inspect known asymmetric pixels, IDs and dimensions.
- [Grammar fixtures](../conformance/grammar-v2.cases) include all five names and
  rejected names/types/keys/versions. Ninety stored verdicts agree with an
  independent JSON Schema validator and the Rust parser.

The 2026-10-24 qualification includes 325 passing foundation/editor tests (0 ignored), all 47 web
unit tests, CLI checks, strict workspace all-target and wasm Clippy, formatting
and source-length checks. Crop's existing tests remain regression gates. No
benchmark improvement, browser interaction or native device result is claimed for
this engine tranche. Generated logs are stored under ignored `docs/validation/`.
