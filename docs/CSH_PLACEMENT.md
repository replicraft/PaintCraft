# CSH placement policy

Studio can convert a qualified custom shape to pixels using **File > Shape
resources > Place shape…**. The current workflow is an explicit numeric form
with X/Y, width, height, fill color, preview and conversion consent. Selecting a
resource or looking at its outline does not paint the document.

Studio also offers **Use as shape tool…**. A separate review shows a sample-size
filled preview and requests consent for converting future drawn shapes to
pixels. After approval, drag the preset onto the canvas in the current color.
Drag backwards or hold Shift for a square target box. Its preview and release
use the same captured resource, preset, policy, RGBA color, layer and endpoints.
The sample preview does not pretend to set a future drag's position or size.
Each accepted stroke records one Shape edit, without another consent prompt.

The toolbar shows the selected resource name and drag instructions in place of
stock shape options. Selecting a stock tool, including Rectangle again, leaves
the imported-shape tool. The color picker's Previous setting restores its exact
source after sampling; a missing source is refused. Temporary Space-pan only
changes the view. Cancelling or invalidating another adoption review retains
the previous tool. Classic and Studio keep distinct document/tool ownership.
These workflows do not complete the unsupported CSH profiles or wider roadmap.

## Qualified geometry

`CubicFillV1` uses the existing tiny-skia filler and the editor's existing mask
commands. It admits exactly one closed path with 2 through 256 knots, no
operation or Combine, and absent/false initial fill. EvenOddLegacy and EvenOdd
map to even/odd filling; NonZero uses winding filling. Incoming/outgoing controls
and the final closing cubic are evaluated, including two-knot curved shapes.

Multiple subpaths, open paths, Exclude/Subtract/Intersect, outside initial fill,
clipboard placement metadata, malformed topology and larger geometry remain
unavailable for placement. These forms remain inspectable/exportable resources;
they are not replaced with a guessed polygon, stock shape or union-only fill.

Every normalized control coordinate must be in [-1, 2]. Output sides are
1 through 4096, with at most 4,194,304 pixels and 262,144 coverage spans.
Canvas drag boxes use inclusive integer pointer endpoints. Browser drags have a
smaller limit of 262,144 pixels, checked before raster allocation on preview and
release. The entire oversized request fails visibly; no partial mask,
downsampling or stock-shape fallback is published. Native drag requests retain
the explicit-placement pixel limit.

Positions are signed integers within the shared coordinate policy, including
the right/bottom extent. Invalid/overflowing dimensions and positions refuse
preparation. Negative positions clip normally.

## Rasterization and clipping

For each signed 8.24 coordinate, the renderer receives:

```text
x = f32(f64(source_x) * target_width / 2^24)
y = f32(f64(source_y) * target_height / 2^24)
```

One normalized unit maps to the geometric right/bottom edge of the target box,
not its final pixel center. The source bounds origin is retained as metadata;
chosen X/Y controls the painting position. Width/height initialize from source
bounds, limited to the admitted output size.

Float conversion can merge distinct fixed-point coordinates. The original
resource keeps its exact integers, record order, flags and bounds. Placement
is an acknowledged raster conversion, not a precision-preserving vector edit
or a claim of Adobe rendering parity.

Tiny-skia fills an opaque white path with antialiasing into transparent scratch
pixels. Its alpha becomes mask coverage. Filling is clipped to the local target
box; source content outside that box is absent from the derived mask. The mask
is translated to X/Y and clipped to the canvas, target-layer extent and active
selection. Existing source-over painting applies the chosen RGBA color, with
alpha rounded from `color_alpha * coverage / 255`. Existing layer properties
and canvas ground are preserved. Imported shapes currently provide a filled
mask; source-compatible outline/stroke styles are not implemented.

## Review and atomic publication

Numeric placement requires a displayed preview from an earlier frame and explicit
conversion consent. Position, size or color changes clear approval. Resource
identity/epoch/index, originating document snapshot/ticket and selected target
are checked before publication. Stale previews, missing/hidden targets,
cancellation and failed admission cannot publish a partial placement.

A placement records one Shape history entry. An empty document gets its first
transparent canvas-sized layer and mask in the same transaction. Fully clipped,
zero-opacity and other ineffective masks leave no new layer or undo entry.
Existing selections and compositing use the same mask executor as other tools.

Native drag preview and mask transaction preparation use the existing workers.
Explicit numeric/adoption preview rasterization currently runs on the UI thread,
like the SVG review workflow. Browser conversion and execution follow the current synchronous
editor architecture. Tiny-skia fill has checks before and after the call, with
no interruption inside it. No interaction-latency or intra-call cancellation
guarantee is made.

## Owned buffers and source protection

The caller supplies a SharedBudget. Scratch pixel bytes and a provisional path
input allowance are declared before allocation. Span vectors declare capacity
before growth and retain their BudgetAccount with immutable Arc storage.
Clones share the allocation and charge. Translated coverage is independently
budgeted. Preview RGBA bytes own a lease; frontends retain a separate declared
copy lease with their cached image. Quota failure, cancellation and last-owner
drop release these declarations.

This covers the owned buffers and declared handoff. It does not bound every
internal tiny-skia allocation, GPU memory, allocator overhead or process RSS.

Native CSH acquisition retains the same opened regular file whose bounded bytes
were decoded. Loaded resources and pending previews remain protected from image
exports and project saves. Accepted placements retain frozen source identities
with their document across normal PCND saves, library replacement, other tabs
and parked interface workspaces. Renaming the source does not permit replacing
its original file through a new alias. Normal Save to a separate bound painting
still works. This is source ownership protection, not proof of race-free
traversal on arbitrary filesystems.

Browser uploads do not grant writes to the original file. Resource exports and
project downloads remain separate operations.

## Focused evidence

The repository-authored [placement specimen](../crates/services/paintcraft-formats/tests/csh-fixtures/README.md)
independently packs a rectangle, a cubic lens, a subtract preset and a multiple
path preset. Backend tests compare analytic interiors, exact rectangle pixels,
orientation, closing curves, distinct fill rules, signed clipping, float
precision, refusal and cancellation. Ten backend tests pass, including quota
failure, retained-clone/drop and preview/copy lease release.

Three Shape editor tests and five Brush regressions pass after sharing first
layer assembly. A separate builder test verifies unchanged brush alias/name and
explicit shape alias/name. Two shared widget tests cover invalid geometry and
unchanged translucent colors. Seven native placement/review tests exercise
translated controls, exact signed pixels, one undo, hidden/stale/cancelled review,
normal saving and real input-alias protection for numeric and adopted tools.
Two native controller tests verify worker preview/release equality, ineffective
masks and exact picker-source restoration. Seven browser resource/review tests
cover the numeric form, drag adoption, unchanged title, staleness, separate
Classic ownership and narrow controls; two controller tests cover whole-request
pixel limits, cancellation, missing sources, picker restoration and hidden layers.
Four shared resource-gesture tests cover captured requests, Shift, backwards
boxes, subpixel input, cancellation, explicit stock selection and view-only pan.

Full host validation passes 458 native tests with 9 ignored, and 185 browser
tests. All 27 tools unit tests, 3 picker integration tests and 8 Studio tests
pass. English/Japanese controls, all seven language catalogs, strict host checks
and a real production Wasm build pass. Browser admission bounds are correctness
limits, not an interaction-performance qualification.

A local headless Chrome production run uploaded CSH, displayed an unsupported
Boolean assessment, previewed the cubic at 64 × 48, blocked unapproved placement,
then placed, undid/redid and downloaded a 789-byte PCND without page errors. CLI
reopening/rendering matched 2,400 independent analytic interior pixels and kept
the canvas outside the target box unchanged. Recovered alpha area was 1840.494
against analytic area 1843.2, within the declared tolerance of 8. This does not
claim independent exact edge-antialias values or original-application parity.
Local artifacts/scripts are under ignored
`docs/validation/format-specialists-2026-10-25/csh-placement/`.

A second production run selected the drag tool with separate preview/consent,
showed the named preset in both toolbar rows, cancelled a live preview with
Escape, drew the cubic, and compared identical preview/release crops. Undo
restored the original crop; Redo restored the painted crop. Its 801-byte PCND
reopened through CLI. The observed pointer box was (99, 80), 64 × 47; 2,340
analytic interior pixels matched, recovered alpha area was 1801.365 against
1804.8 with tolerance 8, and pixels outside the target box stayed unchanged.
The browser's public unsaved-work guard counts held gestures; cancelling the
preview restored its clean state. This run had no page errors. Its screenshots,
Wasm SHA-256, harness and independent verifier are under the ignored sibling
`docs/validation/format-specialists-2026-10-25/csh-drag/`.

Remaining work includes multiple/open/Boolean placement, persistent resource
catalogs, additional source profiles and original-application qualification. The wider roadmap remains open.
