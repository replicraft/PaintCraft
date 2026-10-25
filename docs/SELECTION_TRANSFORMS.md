# Selected-pixel rotation and mirroring

Classic's Rotate/Flip rows and the shared Edit actions now execute exact right/left
90°, 180°, horizontal and vertical transforms. Desktop routes through its editing
worker; web uses the same commands synchronously. With no selection, the whole
canvas and complete layer extents use [canvas transforms](ORTHOGONAL_TRANSFORMS.md).
With a nonempty rectangular selection, only the active layer's selected pixels
transform. Active-empty selections disable these controls. Custom angles remain
unavailable.

## Defined transaction semantics

`raster.transform_selection` is an additive v2 operation with `layer_ref`,
`transform` and `clear`. Names match `canvas.transform`; `clear` is exact RGBA.
An inactive selection is an input error. Active-empty is a no-op, while missing
layer references still fail. V1 rejects this operation.

The source rectangle is the selection intersected with the canvas. Capture its
original samples before writing, clear its intersection with the target layer,
then replace destination samples from the capture. The destination retains the
source rectangle's top-left position; quarter turns swap its width and height.
Replacement is exact straight RGBA, including RGB under zero alpha, with no
resampling or source-over blending. Sampling outside the finite target layer
returns transparent black, independently of the layer's implicit default.

The layer expands/rebases as needed to contain its previous extent and the full
destination. Original pixels outside the source/destination union remain intact.
New padding is transparent; the original default pixel remains unchanged. Pixels
placed outside the canvas remain stored in the layer. The resulting selection is
the destination intersected with the finite canvas. Canvas dimensions, layer IDs,
stack order, names, opacity, visibility and all other layers remain unchanged.

Classic captures the project canvas ground as `clear`, with the established white
fallback; Studio uses transparent black. Whole-canvas quarter turns remember the
new authored startup size after publication in Classic/web. Selection transforms
keep the canvas size unchanged. Undo/Redo leave that last-authored preference alone.

One immediate transaction contains source removal, destination replacement and
selection state. Undo/Redo restore recorded snapshots exactly. These are discrete
commands: repeated transforms operate on the latest published pixels. A floating
selection retaining its original source across repeated interactive manipulation,
freeform rotation/handles, other masks, pivot/profile qualification and complete
Windows/Paint.NET native semantics remain work. Top-left anchoring here is a defined
integer operation, not an independently verified native profile pivot.

## Source-bound patch planning

`MappedPlan::prepare_patch` captures one immutable logical source and a piecewise
source-coordinate/constant-color mapping. It uses metadata-charged candidate/output
tile maps, without decoded preflight buffers or per-pixel paint descriptors.
Candidates include sparse source cells, transformed source cells and constant
clear/padding regions that differ from the implicit default. A huge uniform no-op
can require zero samples and decoded storage.

Logical readers prepare bounded coordinate scratch once when a conditional patch
appears in their lineage. Each read walks that lineage iteratively, selects source
coordinates before evaluating ordered paint and uses no per-pixel allocations or
recursive source calls. The existing affine-only reader path remains separate.
Scratch is charged to the reader's metadata account and synchronized per reader.

Mixed batches retain earlier paint, transforms, floods, duplicates and crops.
Replay uses the normal complete-batch simulated decoded peak reservation before
allocating private output. Failure/cancellation publishes nothing. This first
patch replay reconstructs the affected layer's sparse output cells, including
unchanged non-default cells; it does not yet share untouched output tiles.
Candidate samples across crop, canvas and selection transforms share the provisional
16,777,216-per-batch work ceiling. Extent, coordinate, metadata and decoded admission
limits still apply. These are logical limits, not physical memory or latency claims.

## Qualification (2026-10-24)

- [Raster patch oracles](../crates/foundation/paintcraft-raster/tests/selection_patches.rs)
  independently extract/clear/rotate/paste dense arrays for all orientations,
  overlap, negative/outside source coordinates, raw transparent RGB and expansion.
  Twelve mixed conditional/paint/whole-transform prefixes, crop and expansion
  exercise logical readers without decoded allocation. Huge uniform no-ops,
  work-limit denial, cancellation and extreme typed coordinates are covered.
- [Engine](../crates/editing/paintcraft-editor/tests/selection_transforms.rs)
  checks selected hidden targets, untouched other layers, metadata, source holes,
  off-canvas destination storage, transparent padding/rebasing, exact history,
  active-empty/inactive behavior, invalid targets, mixed batches and pre-allocation
  denial. [CLI](../apps/paintcraft-cli/tests/cli/selection_transform.rs) checks wire
  execution and PCND reopening. All 105 independent schema verdicts agree with
  the parser.
- [Native GUI](../apps/paintcraft/tests/gui/transforms.rs),
  [worker](../apps/paintcraft/src/app/editing/tests.rs) and
  [web](../apps/paintcraft-web/src/paintcraft-web/crop_tests.rs) exercise real controls, one-step
  publication, Undo, target selection, whole-canvas pixels and authored-size policy.
  The full native suite passes 258 library tests (8 ignored) and 217 GUI tests
  (45 ignored); web passes 48 tests. Foundation/editor and shared UI/tool suites,
  strict workspace/wasm Clippy, formatting and source-length checks pass.
- An isolated headless Brave/WebGPU instance drew ink, rotated a 30 × 40 selection,
  undid it, deselected and rotated the whole canvas. Downloaded PCND states were
  reopened and rendered by the CLI. Independent dense PNG oracles match every
  output pixel for selected and whole-canvas rotation: canvas sizes are 1000 × 486,
  1000 × 486 and 486 × 1000. Undo restored the sampled original screen pixels;
  no JavaScript exceptions occurred. This qualifies that browser path, not other
  browsers, native menu/pen/IME handling, reference fidelity or the T22 matrix.

Generated logs, scripts, project copies and reviewed captures remain in ignored
`docs/validation/selection-transforms-2026-10-24/`. T14/T15 remain partial.
