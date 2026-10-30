# Persistent point-filter masks

Status: implemented and locally qualified, 2026-10-30.
Instances retain masks through history and PCND4. Typed batch v3, CLI and MCP
can capture selections, set numeric coverage, enable masks and remove them.
Desktop and browser Studio expose selection capture, enable/disable and removal
through each filter's actions menu. Their parameter dialogs show whether the
filter uses a saved selection, an empty mask or the whole layer.

`paintcraft_raster::coverage_mask::A8Mask` stores fractional fields as canonical
repeated row bands with charged shared storage. Selection capture, source
transforms, shared views, admission refusal and cancellation have focused
coverage. `paintcraft_effects::mix_point_coverage` implements the rule below,
checked against independent real interpolation for all 16,777,216 byte
pair/coverage combinations. Both initial point filters evaluate retained masks.

The local renderer suite passes 104 tests, including 34 required offscreen GPU
controls on Apple M1 Pro/Metal. Two new GPU groups compare masked presentation
and display reductions against independent literal pixels. Seven CPU groups
cover fractional coverage, ordered filters, tile boundaries, negative origins,
cache invalidation, sparse implicit pixels and cancellation. This establishes
the tested CPU-evaluated GPU upload path, not GPU filter kernels or other GPUs.

Editor tests cover atomic selection capture, Undo/Redo, crop, orthogonal
transforms and selected-content rebasing. CLI and actual stdio MCP tests exercise
PCND4, immutable source pixels and metadata-limit rollback; 26 independent
JSON Schema controls validate genuine outputs and reject malformed variants.
Binary reader, worker, frontend and export qualification retain their separate
scope; these results do not establish physical-device or full catalog parity.

The complete local format suite passes 767 tests, with 28 existing ignored
tests. Bounded PCND4 replay passes 6,168 raw and 6,168 checksum-aware mutation
cases, separately inventoried from legacy corpora; this is not coverage-guided
or sanitizer qualification. Native and browser host suites exercise mask
publication, active empty selections, history, private drafts, stale edits and
real PCND4 opening, with strict and Wasm checks.

Production Chrome checks cover five visible-control lifecycle groups and four
fractional-mask presentation states. Downloaded projects are decoded
independently; removing the last mask returns PCND3, Undo restores PCND4, and a
fresh application reopens and byte-identically saves the project. The fractional
case binds full-resolution Worker pixels to the exact serialized source SHA,
state, region and null operation, and checks literal screenshot colors at the
same canvas marker through mask disable/Undo/Redo. Saved projects alone do not
prove a live selection was cleared; host tests cover selection independence.

Ten masked persistent-Worker cases and 66 browser export outcomes exercise
private previews, source admission, default bake refusal, metadata authority and
exact masked output. Eighteen new encoded copies are decoded independently.
Sixteen legacy render and 24 unmasked export controls pass on the same production
assets. These results qualify the two initial point filters and fixed masks;
mask painting/linking, layer masks/styles and the remaining catalog are open.

This extends [Palette integration](PALETTE_INTEGRATION.md) and follows the
canonical [effect-region contract](../temp/paintcraft-implementation-spec-v5-hyperfine/docs/architecture/EFFECT_REGIONS.md).
It does not change the deliberately binary live-selection type.

## Coordinates and coverage

Each filter instance may own an enabled, immutable A8 coverage field. Coordinates
are integer pixel coordinates local to its layer's finite source raster, before
layer translation. Outside the mask's finite domain, coverage is zero. Within
the domain, implicit coverage is explicit and persistent, including 0 and 255.
Uniform rectangles must not require dense canvas allocation.

No mask means full coverage. A disabled mask also means full coverage, while
retaining its exact payload for later enablement. An enabled all-zero mask means
the filter has no visual effect; it is not absence of a mask. Disabled filters
retain their masks without evaluating either the mask or the algorithm.

For each enabled point filter, let `p` be its incoming RGB channel, `q` its
unmasked candidate channel and `m` its mask coverage. Output is exactly:

```text
((255 - m) * p + m * q + 127) / 255
```

Division is integer division. Evaluate and mix once per filter, in stack order.
Alpha remains the exact incoming alpha, including zero; hidden RGB follows the
same RGB rule. Coverage does not attenuate layer opacity or alpha. Layer opacity
and source-over happen after the complete stack. A later alpha-changing or
spatial filter needs its own explicit contract before adopting this rule.

## Capturing a selection

The command captures the owning session, document state/publication epoch,
layer UUID, layer origin and source extent. It freezes a selection snapshot
against that base and converts document coordinates to source-local coordinates.
Later selection edits, clearing selection, changing tools or moving the layer
must not mutate the saved field.

Support rectangular, arbitrary binary, disjoint and inverted selections, as well
as genuine fractional A8 fields at the raster boundary. Inactive selection and
active empty selection remain different: the UI must offer an explicit
whole-layer choice for the former; the latter produces zero coverage. Never
silently turn an empty selection into a whole-layer mask.

Selection capture is admitted and cancellable. Share immutable storage only
when its ownership, accounting and coordinate view are valid; otherwise copy
through a checked builder. Do not retain a reference to mutable host selection
state, flatten a large sparse field, or walk every implicit canvas pixel.

## Source transformations

| Edit | Mask behavior |
|---|---|
| Move layer / change its document origin | Keep source-local coverage unchanged. |
| Paint or erase source pixels | Keep mask coverage unchanged; evaluate the updated source through it. |
| Move or transform selected pixels | Keep the mask fixed to the layer's coordinate space. If the operation rebases the layer's local origin, compensate the mask view by the same rebase so its document position stays fixed. |
| Change canvas bounds without remapping source | Keep coverage unchanged. |
| Crop a layer's source to local rectangle `R` | Clip the field to `R`, then translate by `(-R.x0, -R.y0)` with the source. Apply the same mapping in preparation and replay. |
| Crop removes the entire layer source | Preserve the retained filter intent, but replace active coverage with the canonical zero field for the replacement transparent source. |
| Orthogonal rotate / flip source | Map coverage by exactly the same integer pixel permutation as the raster; use pre-transform source dimensions. |
| Duplicate layer or filter | Share immutable coverage payloads and preserve semantics; allocate the required new layer/filter identity. |
| Resize or arbitrary-angle resample | Require a declared coverage sampling/border policy and checked source mapping. Until implemented, refuse masked remapping atomically rather than leaving a misregistered mask. |

Disabled masks follow these transformations too. Otherwise re-enabling a mask
after a crop or rotation would incorrectly shift its coverage. Transform views
must stay bounded; repeated transforms must not create an unbounded wrapper
chain. Preserve fractional coverage through exact orthogonal operations.

## Ownership, identity and persistence

The immutable A8 raster primitive owns fallible allocation and byte reservations.
Account actual retained container capacity and coverage payloads, including
construction scratch. Clones share one storage reservation; new storage acquires
a new reservation before allocation. Cancellation or denial releases private
work and leaves the document, aliases and history unchanged. Shared reservations
describe charged resources, not a process RSS guarantee.

Document instances own the optional enabled mask and its immutable field.
Descriptor updates, enable/disable, duplication and reorder must preserve it.
Cached stack identities include the mask's presence, enabled state, finite
domain, implicit coverage and canonical field identity, including masks on
disabled filters. Equal semantic fields cannot depend on allocation addresses,
hash-map iteration order or spare capacity. Owned-byte accounting remains
separate from semantic hashing.

History records the full instance and retains coverage ownership through hot,
cold and restored states. `filter_mask_bytes` deduplicates shared resident
coverage allocations. Coverage remains resident when raster history spills;
RGBA spill tests do not establish spilling mask payloads.

PCND4 declares `layer-filter-masks.v1` and is selected whenever any instance
stores a mask, including disabled and all-zero masks. Without a stored mask,
the writer retains the existing PCND1/2/3 selection and byte profiles. A v4
file without any stored mask is noncanonical. Unknown mask codecs are refused
even when the instance or mask is disabled.

After each descriptor, v4 records mask presence, enabled state, codec
`a8-bands-v1`, a signed finite domain and canonical repeated row bands. Positive
run widths and nonzero coverage omit implicit zero areas. Ordered bands/runs
cannot overlap; adjacent equal records must merge. Reader and writer both
enforce aggregate document limits of 262,144 bands and 262,144 runs, alongside
the unchanged 8 MiB wire metadata limit. Shared host payloads serialize per
instance, so admission counts each decoded instance independently.

`ReadAdmission` carries separate caller-owned source-metadata and coverage
realms. The reader validates borrowed mask records before allocating parse
scratch and retained coverage, with aggregate work/count and peak byte limits.
Default selections and filter coverage share one 256 MiB resident realm;
explicit caller realms remain injectable. These are charged-resource limits,
not measurements or guarantees of process RSS.

Typed batches use `paintcraft.batch/v3` for `filter.mask.set`,
`filter.mask.from_selection`, `filter.mask.set_enabled` and
`filter.mask.remove`. Earlier closed batch schemas refuse these tags. Inspection
returns enabled state, domain, canonical digest and band/run counts, rather
than dumping coverage payloads. Masked CLI and MCP inspection use separate
closed v2 schemas; unmasked outputs retain their established profiles.

Editable-copy bake consent includes mask semantics in its frozen inventory.
Foreign layered exports that cannot preserve a mask refuse by default. Explicit
flat baking evaluates coverage once, preserving the open project's source and
editable stack. Changing a mask invalidates previous export authority and caches.

## Rendering and qualification

All appearance consumers need source-local coordinates: CPU flat rendering,
display reductions, GPU source uploads, thumbnails, visible sampling and
export/print/clipboard. A uniform source span can cease to be uniform under a
mask; evaluating the filter once per old source span is incorrect. Implicit
source pixels inside finite bounds remain part of the filtered appearance.

Required controls before claiming persistent-mask support:

- Literal 0/1/127/128/254/255 coverage with all alpha extremes and hidden RGB;
  ordered multiple filters must distinguish mixing once from double application.
- Uniform, sparse, fractional, inverted and disjoint fields; negative layer
  origins; mask edges across raster tiles; finite outside coverage.
- Private selection capture, source painting, duplicate/update/reorder and
  independent later selection changes.
- Crop/rotate/flip followed by Undo/Redo, cold restoration and PCND reopen;
  compare every sample and source pixel with independent mappings.
- CPU flat, regional/reduced display and actual GPU-presented pixels agree;
  stale preview results and stale cached frames cannot return after a mask edit.
- Cancellation at observed construction/traversal checkpoints, exact admission
  refusal, shared reservation lifetime and late compound-batch rollback.
- Default foreign-copy refusal, explicit bake consent, consent revocation on
  mask changes, and unchanged source/history after exported copies.

A tested A8 primitive alone is a foundation. It does not establish document,
frontend, persistence or physical-device qualification.
