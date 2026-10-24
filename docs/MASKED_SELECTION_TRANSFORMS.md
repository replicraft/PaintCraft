# Free-form selection bounds, crop and orientations

Classic 7/10/11, Studio and browser Classic now show sampled selection dimensions
and enable Crop, Fit Selection, rotate and flip for a proven nonempty binary mask.
All five existing orthogonal operations work: clockwise/counterclockwise quarter
turns, a half turn and horizontal/vertical flips. This is immediate integer pixel
editing, with no resampling or claim of matched native Paint behavior.

## Bounds and feedback

`BinaryMask::bounds_checked` returns the smallest half-open rectangle containing
selected samples inside a requested clip. A finite mask domain is not its selected
bounds. Empty active masks produce no bounds and display 0 × 0; Inactive has no
selection dimensions. Inverted and clipped masks use their actual logical samples.
Uniform full/empty masks cost one work unit. Empty-default sparse fields inspect
stored exception tiles, while full-default fields search boundary lines and skip
zero tile bands only after proving every crossing tile empty.

Traversal is cancellable and admits no partial result. The mutable-work variant
shares an allowance across engine operations. Host `BoundsCache` retains one exact
immutable mask-view/clip key and its result under a fixed 2,097,152-work policy.
It caches deterministic work failure, clears when the selection/document is absent
and releases the previous mask owner on replacement. A failed extraction omits
unproven dimensions, disables dependent view/image actions and shows the translated
bounds-unavailable tooltip. It never substitutes the finite domain or stale bounds.
The cache declares no additional pixel or geometry payload allocation.

Fit Selection uses the sampled bounds without publishing edits, moving the mask or
placing editable text. Browser and desktop use their own view state. UI capability
checks reuse the cache; they do not scan the field every frame.

## Selected pixel transforms

The source is the chosen layer's frozen logical snapshot, including earlier edits
in the same batch. The transform anchor is the top-left of tight selected bounds
intersected with the canvas. Quarter turns swap that rectangle's dimensions.

Only selected original samples are cut, using the frontend's explicit clear color.
The destination receives a sample only when its inverse-mapped original mask sample
is selected. Destination holes fall through to the original source: previously
selected pixels there remain cleared, while other pixels keep their exact original
RGBA. A pasted selected sample wins over an overlapping original cut. Sampling
outside a layer's finite extent yields transparent RGBA, including for layers with
nontransparent defaults. Other layers and the target layer's metadata are preserved.
Hidden RGB, negative origins and valid off-canvas stored pixels remain exact.

The mask undergoes the same integer orientation. Its new selection view is clipped
to the canvas, while the layer can retain transformed pixels beyond the canvas
within the existing extent policy. Whole-canvas commands orient the complete layer
extents and map active mask coverage in canvas coordinates. They differ from
transforming only selected pixels on one target layer.

Preparation admits each transformed mask and retains it once. Replay reuses those
immutable fields and the compiled selection/layer geometry, keyed by operation
index, instead of constructing a second mask or sampling a progressively edited
source. Undo/Redo restores exact pixels and mask views. Failed preparation,
publication, history admission or stale ownership leaves the published state intact.

Mask sampling, run writes and finalization share the engine's selection-work
allowance with other mask-filter work. Mask construction retains its explicit byte,
tile and run bounds. Mapped pixel operations share a 16,777,216-source-sample limit
per batch and retain existing decoded/metadata admission. Huge uniform masks stay
metadata-only; nonuniform orientation can fail its bounded scan/construction policy.
These are logical payload/work bounds, not process RSS or wall-clock guarantees.

## Crop

The tool adapter derives a crop rectangle from actual selected samples. An empty
mask cannot supply Crop To Selection. The canvas crop itself is geometric and
retains all layer content inside that rectangle, including unselected holes, while
discarding outside content and clearing the selection. An explicit typed canvas
crop remains independent of the active selection, including an active empty mask.
Crop does not perform a color-keyed cut or a floating-mask extraction.

## Evidence and remaining scope

Independent small-field pixel scans check tight bounds through all binary 3 × 3
fields, inversion, clipping and translation. Additional cases cover sparse huge
fields, zero tile bands, dense boundary rows, shared cache lifetime, work denial
and every observed cancellation checkpoint. Independent coordinate tables check
orthogonal mask samples and inverse identity.

Mapped patch tests exercise 160 orientation/clip/inversion/clear combinations,
finite off-layer samples, hidden RGB, sparse 256-pixel seams, exact rectangular
compatibility, ownership, quotas and cancellation during preparation and replay.
Editor tests use a separate dense cut/paste and mask-set oracle across 30 target
origin/orientation/clear combinations, plus compound transforms, paint, flood and
whole-canvas orientation. They verify history, late admission denial, cancellation,
private charge release and stale publication. CLI tests reopen actual PCND output
and compare every pixel for all five selected orientations.

Native and browser tests compare sampled dimensions, Fit zoom/centering and actual
rotate/flip/crop actions with exact Undo, retaining editable text during view-only
actions. Existing lasso GUI matrices cover seven languages across native Classic
7/10/11 and Studio (28 cases), and browser Classic (21 cases), including inverted
dimensions and cache invalidation through Undo/Redo.

Floating movement, transparent color-keyed selection, fractional mask combination,
custom-angle resampling, native fidelity and device/performance qualification
remain separate work. T14/T15 remain partial. Session selections remain outside
PCND persistence. Local raw validation logs are retained under ignored
`docs/validation/masked-selection-transforms-2026-10-24/` and
`docs/validation/selection-bounds-2026-10-24/`.

## Completed validation, October 24, 2026

The full unified workspace regression run passed 1,435 tests with 75 explicit
ignores across 124 result summaries:

```sh
cargo test --offline --workspace --no-fail-fast -- --test-threads=4
```

Strict workspace all-target Clippy, Wasm checking, formatting, source length and
all 930 Fluent message mappings across seven languages pass. The actual browser
release distribution and debug macOS bundle were rebuilt; the nested app passes
strict deep code-signature verification. These builds do not establish native
Paint fidelity, physical-device behavior, masked interactive latency or release
signing qualification. The full roadmap remains incomplete.


## Floating selection follow-up, October 24, 2026

[The floating selection tranche](FLOATING_SELECTIONS.md) now implements exact
integer movement and transparent RGB-keyed placement with frozen source pixels,
private previews, bounded admission and one recorded change. Native Classic/Studio,
browser Classic and CLI have independent pixel and lifecycle evidence. Remaining
fractional, custom-angle, clipboard and native qualification work stays open.
