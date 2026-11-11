# Custom rotation

The engine supports arbitrary clockwise rotations of the whole document or a
captured selection. The frontend controls and CLI discovery are a separate
integration step; the engine checkpoint below does not claim they are available.

Whole-document rotation uses one common canvas pivot for every layer, including
hidden and off-canvas layers. It translates the exact rotated minimum to the
original top-left before rounding the output bounds outward. Layer identities,
ordering, opacity, blend mode and filter descriptors survive. Own-layer and filter
masks transform with their layers, including disabled masks. This transforms
retained source pixels rather than baking the filtered composite.

Selected rotation affects only the captured target and selected pixels. Holes and
unselected pixels survive; old selected pixels clear using the captured ground
color. The selection is clipped to the canvas, while off-canvas transformed pixels
remain stored. Identity and empty-selection requests do not create history.
Exact cardinal rotations retain the existing lossless orientation paths.

Other angles use one immutable-source nearest-neighbor pixel-center mapping.
Checked integer geometry controls bounds and sampling. Commands carry a pair of
Q30 cosine/sine integers, with denominator 1,073,741,824 and near-unit validation.
The degree adapter accepts finite angles from -360 through 360 and uses the
already pinned `libm` once; exact cardinals bypass trigonometry. The actual stored
determinant is inverted, rather than assuming rounded coefficients remain exactly
unit length. No rotation fields are added to the document or PCND format.

The additive `paintcraft.batch/rotation-v1` grammar admits `canvas.rotate` and
`raster.rotate_selection`. Both require `coefficients: [cosine, sine]`; the selected
operation also requires `layer_ref` and `clear`. The envelope retains the exact
sorted filter and layer-feature requirements of the existing style-aware grammar.
Earlier batch grammars remain closed to these operation names.

Preparation uses ordinary source snapshots, metadata admission, complete decoded
peak reservation and atomic publication. Destination visits share the existing
16,777,216 transform-read allowance across a batch. Mask limits and cancellation
remain enforced. Stored coordinate Noise retains its existing bake requirement.
Failure leaves the published source and history unchanged.

The local engine checkpoint passes 810 Core/Commands/Raster/Editor tests with no
ignored tests, plus strict native all-target and Main/both-Worker Wasm Clippy,
rustdoc and formatting.
Independent rational fixtures cover bounds and inverse centers, complete RGBA,
hidden RGB, off-canvas layers, both mask types, selected holes and rebased mask
fields. Tests also cover original read limits, cancellation, exhaustion, following
paint and exact Undo/Redo. The initial test fixture's unsorted selection runs and
the one-entry offline lockfile correction remain recorded under
`docs/validation/rotation-engine-root`.

This is a provisional correctness baseline. It does not claim historical Paint
resampling equivalence, measured performance, all-resource admission or physical
memory bounds. Frontend, CLI and fresh browser qualification remain required.
