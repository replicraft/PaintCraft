# Finite selection masks and editing coverage

`paintcraft_raster::selection::BinaryMask` now supplies immutable sparse binary
R8 coverage for the selection work required by T14/T15 and
[the selection contract](temp/paintcraft-implementation-spec-v5-hyperfine/docs/architecture/SELECTION_CONTRACT.md).
The storage/kernel foundation now has owned session/history integration. The
canonical `Selection` also carries `Mask(BinaryMask)`, with cheap immutable clones
through worker preparations, recorded history and cold document restoration.
Painting and flood connectivity honor the exact binary field. Desktop and browser
now expose [free-form selection, Invert and true mask outlines](LASSO_SELECTIONS.md).
Transparent and floating selection require their own pixel-placement semantics.

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
  A larger chosen inversion domain is explicitly reframed by the
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

1. Complete native reference qualification for the owned mask snapshots, polygon
   creation and finite-domain inversion now routed through both frontends.
2. Qualify native reference selection behavior and masked interactive performance.
   Binary painting gates and flood connectivity are implemented and tested below.
3. Add mask-specific packing/spill and native pressure/retry services. Recorded
   snapshots already retain exact masks with resident byte accounting and
   cancellation. Session masks remain outside PCND persistence unless explicitly
   promoted to named document data.
4. Carry the original mask with floating selected pixels and interactive transforms.
   Canvas mapping and immediate orthogonal operations now work as documented below.
5. Add transparent color-keyed floating selection. Tight sampled bounds/status,
   native/browser lasso gestures, true boundaries and Invert now work.
6. Expand native profile and device evidence before claiming SEL-01 through SEL-05
   complete. Scoped batch, worker, GUI, CLI and browser-host evidence exists.

The broader Classic backlog is recorded in [Classic follow-up](CLASSIC_FOLLOWUP.md).

## Foundation-only validation, October 24, 2026

All 154 raster tests and 46 core tests passed. Workspace all-target checking and
strict workspace/WebAssembly Clippy passed. The mask suite contains ten tests,
including independent polygon pixels, large-coordinate rational sorting, zero-byte
empty geometry, failure cleanup and shared snapshot lifetime. Source length and
whitespace checks passed. Local raw logs are in the ignored directory
`docs/validation/selection-mask-foundation-2026-10-24/`.

That foundation-only run did not establish engine/session mask behavior, native
latency, history packing or GUI/CLI/web selection parity. See the owned integration
evidence below;
native admission/watch/retry routing for the new mask realm is also still pending.


## Owned session/history integration, October 24, 2026

Version 2 batches now accept `selection.set_polygon` with 3 through 65,536 integer
`{x,y}` points, and `selection.invert` with no additional fields. Version 1 rejects
both operations. Polygons rasterize once over the finite current canvas domain;
compilation retains their admitted immutable results for replay. Inversion
complements within the current canvas after explicitly zero-reframing any smaller
selection domain. With an inactive or empty legacy selection it selects the full
canvas. Rectangle selection and clearing retain their existing v1 behavior.

Canvas placement translates and clips an owned mask without copying its storage.
An expanded canvas does not resurrect samples hidden by an earlier clip. Invert
shares storage when the domain is unchanged; reframing an enlarged domain uses
bounded private construction. The process-default mask payload realm is 256 MiB,
shared by normal admissions. Callers can supply a separate `SharedBudget` using
`Admission::with_selection_budget`; the existing construction/work limits remain.
This is a declared payload limit, not a total application/RSS ceiling.

Selection-only changes advance the publication epoch and recorded history while
retaining the content StateId and native save cleanliness. Existing owner/base
checks reject a late preparation after a selection change. Exact shared views
compare cheaply; separately constructed fields use a cancellable, bounded semantic
comparison. Finite domain is part of field identity; equal samples over different
domains can record distinct selection states. Comparisons of nonuniform fields
can fail on their work bound instead of publishing an unproven no-op.

Hot and cold history snapshots own mask storage. `MemoryUsage::selection_bytes`
deduplicates declared payload across shared, translated and inverted views and
includes it in retention budgets. Packing/spilling raster history does not pack
or evict mask bytes. Preserving retention rejects an unaffordable record atomically;
explicit trimming can release masks when their final reader disappears. External
mask readers still hold their original realm charges after history drops them.

The ownership slice initially rejected painting while a mask was active. The
coverage integration below now enables painting and flood fill. Crop and mapped
orthogonal transforms now follow [explicit sampling and geometry policies](MASKED_SELECTION_TRANSFORMS.md).
Layer operations, canvas placement and Clear Canvas retain their established
semantics. The subsequent frontend integration exposes lasso/Invert with true
mask outlines. Tight sampled dimensions now use a separate derived bounds cache.

New engine tests cover independent polygon pixels, finite inversion, selection-only
Undo/Redo/seek, stale workers, single-construction replay, equal-field no-ops,
resize/reframe, every observed construction cancellation, failed history admission
and failed publication. History tests force actual cold document reconstruction
and verify exact ownership, shared-byte accounting, budget denial and final-owner
release. Schema and raster view tests exercise strict versioning, malformed input,
comparison bounds, cancelled reframing and hidden-sample preservation.


The integrated workspace regression run passed 1,354 tests with 75 explicit ignores
across 96 result summaries. The new targeted CLI tests passed separately (two
process-level cases plus nine independent JSON-schema verdicts). Strict workspace
all-target Clippy, WebAssembly checking and the portable browser release build
passed, along with formatting, source-length and seven-language catalog checks.
Studio layout tests also cover restart, save-gated switching and settings
Apply/Cancel. No live window, native device or matched-reference qualification is
claimed. Raw logs and the source recording are local under the ignored
`docs/validation/owned-selection-masks-2026-10-24/` directory.


## Exact painting and flood integration, October 24, 2026

`PaintPlan::masked_checked` intersects layer-local writes with document-space
binary coverage before the compiler retains them. Every write keeps its original
blend mode, color, 16-sample replacement coverage and ordering. The logical program
and decoded replay consume that same filtered plan; replay does not apply another
selection pass. This central gate covers rectangles, pencils, all brush media,
smooth erasing, shapes/contours, bitmap text and resolved grayscale text runs.
Erase Selection now emits the same replacement command with the mask's domain as
an outer bound; the actual mask field still authorizes every written pixel.

A batch shares a 67,108,864-unit filtering allowance across its writes and rows.
Traversal checks cancellation and constructs output directly under the original
plan's metadata realm and fixed policy. Uniform empty masks produce no writes;
uniform full masks retain a shared plan when all writes lie inside the domain,
or clip rectangles without scanning their pixel areas. Nonuniform fields use
ordered selected runs. Overflow, traversal or metadata denial discards the private
result; it does not publish a partially filtered edit. Construction, flood scratch,
decoded pixels and history continue to carry their own existing limits.

The flood classifier checks eligibility before every source read, including the
seed. Unselected pixels block four-connected traversal, so an unselected bridge
cannot connect selected islands. The compiler reads earlier filtered writes and
frozen duplicate-layer sources. A forbidden seed returns without classifier scratch
or source reads. Unrestricted classification selects its existing specialized
path outside the pixel loop. This source structure is not a throughput claim.

Independent flood BFS checks 147,456 small image/mask/seed/tolerance/origin cases.
Editor tests compare every paint family against an independent binary-field gate,
including inverted/clipped/translated masks, negative layer origins, partial
replacement and overlapping translucent operations. Another 384 editor cases
compare logical walls, layer duplication and flood pixels against dense BFS.
Cancellation at every observed masked-batch checkpoint leaves the owner ticket,
selection, history and declared resource realms unchanged. Decoded admission
failure is also atomic.

Real CLI process tests reopen PCND outputs and check triangle/complement pixels,
disconnected flood islands and forbidden seeds; unsupported mapped transforms
still leave existing output files untouched. Native worker and browser tests check
a concave polygon and its complement through previews, commit, Delete and
Undo/Redo. A delayed native preparation cannot publish after a newer selection-only
change, even though content StateId has not changed.

The workspace run passed 1,373 tests with 75 explicit ignores across 99 result
summaries. Following the final classifier dispatch extraction, editor/flood
regressions passed again; two new native and one browser host tests passed
separately. Final strict workspace Clippy, Wasm checking, portable browser build,
formatting, source-length, translation and whitespace checks passed. Raw logs and
source recording are local in the ignored
`docs/validation/masked-painting-2026-10-24/` directory. Native reference fidelity,
interactive latency, fractional masks, mask packing/spill, floating pixels and
mapped selection transforms remain incomplete. Outlines and lasso controls were
implemented after this run; see the frontend evidence linked above. T14/T15 and
SEL-01 through SEL-05 are not promoted to completed gates by this tranche.


## Tight bounds and orthogonal transforms, October 24, 2026

[The next selection tranche](MASKED_SELECTION_TRANSFORMS.md) implements bounded
sampled bounds, reusable host feedback, Crop/Fit Selection and exact selected/whole
canvas orientations. It preserves mask holes, frozen original samples, exact
history and private admission failure. Floating and transparent selections,
fractional combination and matched native qualification remain unfinished.


## Floating selection follow-up, October 24, 2026

[The floating selection tranche](FLOATING_SELECTIONS.md) now implements exact
integer movement and transparent RGB-keyed placement with frozen source pixels,
private previews, bounded admission and one recorded change. Native Classic/Studio,
browser Classic and CLI have independent pixel and lifecycle evidence. Remaining
fractional, custom-angle, clipboard and native qualification work stays open.
