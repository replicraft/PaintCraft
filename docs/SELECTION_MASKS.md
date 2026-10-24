# Finite selection mask foundation

`paintcraft_raster::selection::BinaryMask` now supplies immutable sparse binary
R8 coverage for the selection work required by T14/T15 and
[the selection contract](temp/paintcraft-implementation-spec-v5-hyperfine/docs/architecture/SELECTION_CONTRACT.md).
This is the storage/kernel foundation. The editor session's existing `Selection`
projection remains Inactive/Empty/Rect, and free-form, inversion and transparent
selection UI are not enabled by this change.

## Field semantics

- A mask is an active finite half-open document domain. Coverage outside it is
  always zero, including for an inverted mask. An all-zero mask is not Inactive.
- The initial mask accepts only coverage 0 or 255. Fractional selection coverage,
  fractional combination formulas and selection/brush alpha interpolation are
  deferred until their policy and independent fixtures are implemented.
- Missing tiles inside the domain use the implicit default. Full/empty fields
  have no tile storage, including a 1,048,576-square domain.
- Exceptions use 256-square tiles with one byte per dense sample. Fully covered
  tiles collapse to a solid byte. A uniform result normalizes to no tiles.
- Inversion complements within the mask's current domain. Inversion, cloning,
  integer translation and clipping share the same immutable tile/index storage.
  A larger chosen inversion domain must be explicitly reframed by the future
  session adapter; expanding a clipped domain must not resurrect stored samples.
- An empty clip returns `None` to represent an active empty intersection. Callers
  must not map that result to the unrestricted Inactive state.
- Ordered row traversal emits merged selected half-open spans, with zero coverage
  outside the finite domain. It needs no row-sized temporary allocation. Callers
  carry a shared work allowance across rows and stage any emitted prefix privately.

## Polygon rasterization

The polygon constructor rasterizes a closed path with the even-odd rule at pixel
centers `(x + 1/2, y + 1/2)`. Vertices are integer pixel-boundary coordinates.
Concave, repeated, degenerate and self-intersecting paths follow that rule; the
resulting samples are authoritative, not a second geometry cache.

Intersections use i128 rational arithmetic. Sorting compares integer quotients,
then bounded remainder cross-products, avoiding an overflowing cross-product of
whole rational numerators at large signed coordinates. Pixel ranges use exact
ceil division. A fallible in-place heap sort checks cancellation and work bounds;
there is no uninterruptible library sort or floating-point boundary decision.
This defines PaintCraft's initial binary kernel, not a native Paint fidelity
claim. Native profile/antialiased-mask qualification remains separate.

## Admission and lifetime

Callers supply a shared byte realm and explicit `MaskLimits`. Dense sample
capacity and index-entry capacity retain ownership-based charges. Polygon
intersection scratch is admitted separately in the same construction scope.
Clones/inverted/clipped views share the storage charge; its final owner releases
it. Failed or cancelled private construction releases its allocation prefix and
leaves existing masks intact. Denials carry the `SelectionMasks` resource class.

The default construction bounds are 67,108,864 work units, 4,096 exception tiles,
262,144 runs and 65,536 polygon vertices. Work includes span validation, written
sample visits, polygon edge scans, intersection sorting and tile finalization.
Traversal has an explicit independent allowance. These are declared capacity/work
policies, not allocator/RSS interception or wall-clock latency bounds. Hash/Arc
control storage, caller-owned input geometry and allocator/reallocation overhead
are outside the declared payload charge.

## Evidence and next integration

`crates/foundation/paintcraft-raster/tests/selection_masks.rs` checks finite full
and inverted million-square fields, negative tile boundaries, shared lifetime
charges, uniform normalization, ordered seam-free rows, failed replacement,
callback cancellation and malformed input. Independent per-pixel ray casting
checks hand-built and 204 deterministic generated polygons; translated and huge
edge-rational cases exercise the core coordinate limit. Scratch/work/byte denial
and cancellation during construction/sorting leave no retained private capacity.

The remaining integration must:

1. Extend session selection snapshots and operation schemas with mask ownership,
   polygon creation and finite-domain inversion; preserve selection ticket guards,
   selection-only history and native dirty-state rules.
2. Gate every painting kernel exactly once by binary coverage. Flood traversal
   must respect mask connectivity while reading, not merely filter a finished fill.
3. Record and restore masks through history, with byte accounting, cancellation,
   cold storage and pressure policy. Session masks remain outside PCND persistence
   unless explicitly promoted to named document data.
4. Translate/clip/reframe selections correctly for canvas and image operations,
   and carry the original mask with floating selected pixels and transforms.
5. Add native/browser lasso gestures and mask-derived boundaries/status, then wire
   Invert and transparent color-keyed floating selection to real shared commands.
6. Add independent batch, native worker, GUI, CLI and browser evidence before
   enabling the corresponding controls or claiming SEL-01 through SEL-05 complete.

The broader Classic backlog is recorded in [Classic follow-up](CLASSIC_FOLLOWUP.md).

## Validation, October 24, 2026

All 154 raster tests and 46 core tests passed. Workspace all-target checking and
strict workspace/WebAssembly Clippy passed. The mask suite contains ten tests,
including independent polygon pixels, large-coordinate rational sorting, zero-byte
empty geometry, failure cleanup and shared snapshot lifetime. Source length and
whitespace checks passed. Local raw logs are in the ignored directory
`docs/validation/selection-mask-foundation-2026-10-24/`.

This does not establish engine/session mask behavior, native latency, history
packing or GUI/CLI/web selection parity. Those require the integrations above;
native admission/watch/retry routing for the new mask realm is also still pending.
