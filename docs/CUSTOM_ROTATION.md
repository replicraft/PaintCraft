# Custom rotation

The engine supports arbitrary clockwise rotations of the whole document or a
captured selection. Classic 11 and Studio have staged angle controls, and CLI/MCP
use the same command path. Native and browser-host tests pass; execution of the
new production website remains a separate pending checkpoint.

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

The consumer checkpoint passes 1,688 desktop, browser-host, shared UI and Tools
library tests, with 40 existing or explicitly gated tests ignored. The real CLI
file test and Tools command test pass, and all 32 real stdio MCP tests pass.
Strict native all-target and Main/both-Worker Wasm checks, 410 web Node tests,
Biome and Rust formatting pass. Actual angle-widget events stage values, Cancel,
reopen and Apply in both native Classic 11 and Studio without an intermediate
publication. Hidden RGB, alpha, exact Undo/Redo, stale selection/owner/epoch,
pending-modal retirement and the original browser four-megapixel bound are tested.
The local receipts are
`docs/validation/rotation-engine-root/engine-final.json`
(SHA-256 `84d63b8deb8fc8137476912556d838218ba8690d51383b6eb0f49c9008cbd7ea`)
and `docs/validation/rotation-engine-root/consumer-final.json`
(SHA-256 `ffb2ccff5716992f989044bc69426a906866bd198465ebd849eb86f763ebce53`).

This is a provisional correctness baseline. It does not claim historical Paint
resampling equivalence, measured performance, all-resource admission or physical
memory bounds. Fresh production browser qualification remains required.
