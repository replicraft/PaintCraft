# Palette integration: persistent nondestructive filters

Status: ordered history, PCND persistence, regional rendering, typed automation
and explicit copy consent support **seven editable profiles**: six point filters
(Invert, Brightness/Contrast, Black & White, Threshold, Posterize, Solarize) and
Box Blur. Fixed per-filter masks remain independent of live selection. Native
and Web Studio share controls. The six-point production proofs below remain
qualified; Box local host/CLI/PCND/required Metal checks and actual production
Worker3/scoped Studio/MAIN DDS/MSP export5 pass finite appearance, consent and
replay checks. MSP has independent exact 2×1 opaque-white pixel evidence. Updated 2026-10-30.
Layer masks, styles and the rest of the pinned 132-entry inventory remain open;
seven profiles do not establish parity or a wholesale port.

Complete five-parameter Levels is the next local integration slice. Typed intent,
hot/cold history, PCND6 catalogs, prepared CPU/Metal rendering, Studio controls and
catalog CLI/MCP receipts are implemented and locally tested. Fresh web production
builds and 20 raw Source4 browser cases pass. Visible Studio lifecycle and browser
export qualification are still in progress, so Levels is not included in the
seven-profile public support claim above. See the [Levels contract](LEVELS_FILTER_FOUNDATION.md)
and [catalog framing](PCND_FILTER_CATALOG.md). No layer styles or remaining catalog
entries are implied by this work.

## Evidence and upstream identity

The reviewed repository is [spiffyfitz/palette](https://github.com/spiffyfitz/palette), commit [`aefc84fe9233e66344147078de866860326d8cfa`](https://github.com/spiffyfitz/palette/commit/aefc84fe9233e66344147078de866860326d8cfa). GitHub reference verification identified root tree `862c118edfc402f66ae34aea34d940b1a7d119c9`. The read-only source snapshot and individual blob hashes are recorded under `docs/validation/palette-source/`.

Evidence labels below mean:

- **Author inventory:** described in [IMPLEMENTED_FUNCTIONS.md](../temp/IMPLEMENTED_FUNCTIONS.md), including 132 conventional filters across 15 categories, ten effect families, nine brushes and 19 blend modes. These counts are not an independently executed catalog qualification.
- **Read source:** directly inspected files in the pinned snapshot establish the stated structure or algorithm. No Palette build, benchmark, original-application comparison or GPU execution was performed for this design.
- **Proposed:** PaintCraft implementation and acceptance gates still required. Source availability does not establish feature parity, exact proprietary output, performance or compatibility.

Pinned [document enums](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/document.rs#L343) independently enumerate **132 Filter variants** and **11 Effect alternatives**, including a Configured wrapper and ten families. This establishes the source surface, not runtime correctness of every entry. The supplied author inventory uses a newer split-module layout in places; the pinned snapshot retains implementations in monolithic `processing.rs`. A differing file path is not evidence that a feature is absent.

| Capability | Evidence actually read | PaintCraft consequence |
|---|---|---|
| Immutable source versus editable processing | [Layer/FilterInstance definitions](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/document.rs#L1313): source raster, ordered filters, stable IDs, enabled state and individual masks | Persist descriptors and masks separately from raster pixels; painting remains an edit of source data. |
| Ordered layer appearance | [processing/layer.rs](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/layer.rs#L530): filters, color overlays, layer mask, inner/outer styles | Adopt explicit stage order; layer styles and backdrop-dependent exterior effects require separate contracts. |
| Invert and Brightness/Contrast | [processing/point.rs](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/point.rs#L212): invert RGB with alpha preserved; contrast gain `(1+c)/(1-c)` | Version encoded-RGB arithmetic, parameter range, rounding and hidden RGB before shipping. |
| Registry and validation | [processing.rs](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing.rs#L490): defaults and contrast range `[-0.99,0.99]` | Share validated descriptors across UI, command schemas and evaluation. Do not admit the singular contrast endpoint. |
| Private parameter editing | [app/processing.rs](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/src/app/processing.rs#L66): isolated candidate, cancellation ticket, session/generation checks; Apply changes document descriptors | A slider preview must not publish history or overwrite source pixels; Apply submits one atomic edit. |
| Appearance caching | [same_appearance](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/processing/layer.rs#L373) and [CPU compositor](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/crates/palette-core/src/composite.rs#L91) | Reuse resolved appearances when dependencies match; presentation request freshness is an additional check. |
| GPU and full catalog | Author inventory describes CPU filter evaluation, GPU composition, CPU export, 2048-side reduced previews and cache budgets | GPU filter kernels are not established by the inspected subset. Do not transplant budget constants or claim all 132 filters from this review. |
| Persistence, mask editing and advanced styles | Inventory lists native preservation and selection-derived masks, but also unfinished mask painting/linking and per-effect masks | These remain separate acceptance gates, not implied by a FilterInstance field. |

## PaintCraft boundaries at the start of integration

[Document/Layer](../../crates/foundation/paintcraft-document/src/lib.rs) initially persisted raster, origin, visibility and opacity, with no filter stack. `same_semantics` compares complete layers. Selection belongs to the session; it is not a persistent document mask. Original source ICC/resolution is provenance, not an active working profile. The implemented stacks and their persistence are described below.

[The renderer](../../crates/services/paintcraft-render/src/lib.rs) composes ordered straight RGBA8 layers with fixed intermediate rounding. Original [cache keys](../../crates/services/paintcraft-render/src/cache/key.rs) used layer geometry/opacity, raster default pixels and source region versions. Integration added stack identities so parameter-only edits invalidate the evaluated appearance.

[Cold history](../../crates/editing/paintcraft-history/src/archive.rs) records explicit layer fields and raster payload references; adding only a live Document field would lose the stack during packing/restoration. Integration preserves the stack in cold history and adds PCND3 for nonempty stacks, retaining legacy v1 and source-metadata v2 profiles. The `paintcraft-effects` crate supplies pure point kernels; persistent meaning belongs to document/history/formats.

The normative [effect-region contract](../temp/paintcraft-implementation-spec-v5-hyperfine/docs/architecture/EFFECT_REGIONS.md) remains authoritative: `required_input` and `affected_output` are separate functions; masks apply once; tiles are not image boundaries; admission and cancellation precede publication.

## Persistent model and arithmetic

Implemented ownership for the point-filter slice:

| Owner | Responsibility |
|---|---|
| `paintcraft-core::types::filters` | Stable kind/profile IDs and validated typed parameters, no renderer/UI dependency |
| `crates/services/paintcraft-effects` | Pure point kernels first, then declared ROI/halo/global-analysis implementations |
| `paintcraft-document` | Ordered immutable per-layer instances: stable UUID, enabled state, descriptor, optional persistent coverage mask |
| `paintcraft-commands` / editor | Typed add/update/toggle/duplicate/remove/reorder operations, validation, atomic preparation and publication |
| render/runtime/history/formats | Evaluation, resource leases/cache keys, snapshot restoration and canonical PCND persistence respectively |

Use a versioned encoded-RGB8 policy, not an assumed color-managed or linear-light result. Alpha is unchanged; **hidden RGB is transformed even when alpha is zero**. Final source-over composition retains its existing transparent-pixel behavior. Original ICC bytes stay attached as provenance and are not activated by a filter.

First descriptors use integer hundredths: brightness `[-100,100]`, contrast `[-99,99]`. For channel byte `x`, brightness `b`, contrast `c`, the versioned exact reference is:

`clamp_round(127.5 + (x - 127.5) * (100 + c)/(100 - c) + 255*b/100)`

Compute this as checked exact rational/integer arithmetic; clamp to `[0, 255]`, then round nearest with ties upward. `b=0,c=0` is byte identity. Reject contrast `100` before storage/evaluation; adding that endpoint later needs an explicitly versioned policy, not infinity, a byte cast or silent clamping. Invert is exactly `255-x` for RGB. Palette's float implementation is a reference for intent, not proof of byte-identical output to this rational policy or another application.

Each filter operates only within its layer's finite source extent. It also transforms the raster's **implicit default pixel** within that extent: iterating allocated tiles alone is incorrect. It does not generate coverage on the unbounded plane. Default-color output must remain implicit where possible rather than allocating every untouched source tile.

An instance mask is persistent source-local coverage, independent of the live selection and layer opacity. Capturing a selection creates a fixed mask snapshot with admitted immutable payloads; later selection edits cannot change the filter. For these alpha-preserving kernels, blend original and candidate RGB once using canonical integer coverage, keeping alpha exact. The [filter-mask contract](FILTER_MASKS.md) specifies zero/full/fractional coverage, outside defaults, capture, transformations, ownership and completion gates. Palette's point path mixes RGB while its spatial path uses a separate mixing helper; that is not a general alpha-changing mask specification.

## Staged adoption and completion gates

### 1. Descriptors and pure kernels

Validated descriptors and scalar pixel/row kernels are implemented in
`paintcraft-core::types::filters` and `paintcraft-effects`. The initial two kernels
retain their exact byte contracts; the four additions have independent numeric
oracles and typed-domain checks. See [all six exact profiles](POINT_FILTER_PROFILES.md)
for required parameters, encoded RGB, hidden RGB, per-stage rounding and mask-once
semantics. Completed historical two-kind tests below are not six-kind UI counts.

All six descriptors use bounded checked row evaluation. State format, input/output extents, supported aliasing, scratch quote and cancellation partition. Reject inconsistent lengths before writing; safe Rust slice borrowing requires disjoint source/output storage. Callers publish only complete accepted results. Start with scalar integer reference; SIMD/GPU are later differential implementations of the same policy.

Gate: independent literal channels including 0/127/128/255, hidden RGB, all alpha extremes, brightness/contrast boundaries, tie rounding, identity, illegal endpoint, hostile geometry, cancellation and unchanged source on failure. This stage alone is **kernel foundation, not nondestructive filter support**.

### 2. Persistent stack, commands, history and PCND

Implemented foundation: each layer has a bounded immutable ordered stack with cached semantic digest. Source tiles remain shared and unchanged; hot/cold history records the stack and accounts for its owned storage. Nineteen document tests and twenty history tests pass. [Six typed editing operations](../FILTER_COMMANDS.md) support atomic history, aliases and inspection in CLI/MCP. Fixed persistent masks are implemented, with their distinct capture and coverage contracts.

Add immutable ordered instances and fixed selection-derived masks; retain source tile sharing. Descriptor order, enablement, IDs, mask bytes/default and arithmetic version enter document equality and semantic identities. IDs remain stable across edits; duplicate receives a new ID; a UI label is not the semantic key.

Prepare typed stack edits against an exact document/session/history base. Validate descriptor count/bytes, UUID uniqueness, target layer, parameter ranges, mask bounds and canonical-save viability before atomic publication. Parameter edits change metadata, not raster payloads. Existing source attachments survive unchanged, and host-owned metadata admission includes stacks/masks, pending candidates, history and reopen.

Extend hot history and cold archive/restoration together, including unique shared-mask accounting and cancellation. Undo/Redo restores recorded descriptors/masks without rerunning filters. Extend duplicate/delete/crop/resize/rotate/merge contracts explicitly: source-local transforms must transform or deliberately bake/refuse dependent masks, never leave them silently misaligned.

[PCND v3](../PCND_FORMAT.md#version-3-live-point-filters) stores the bounded point-filter stack using schema `paintcraft.document/v3` and required feature `layer-point-filters.v1`. Documents without stacks retain exact v1/v2 bytes, including original source attachments. Sixteen independent fixtures, 45 PCND tests and a future-version recovery control pass. Unknown algorithms, even disabled ones, are refused. Masks use the separately qualified PCND4 grammar; stored Box uses PCND5. The v3 profile itself remains maskless and point-only. The fuzz extension keeps its new streams separate from the historical streams.

Gate: unchanged raw raster/tile identities; order-dependent Invert→Brightness/Contrast versus reverse; edit/toggle/reorder/duplicate/remove Undo/Redo; cold spill/reopen; independent PCND fixtures; resource/cancel failures leave history/document/source unchanged. No persistent feature claim until this gate and the renderer gate pass.

### 3. One appearance pipeline for every consumer

Implemented foundation: CPU regional/parallel composition, uniform display reduction, thumbnails, visible sampling/copy and cache keys use evaluated point appearances. GPU source uploads prepare those same CPU appearances for composition and display. The current six-kind checkpoint passes all 108 renderer tests with offscreen Metal required, including 36 GPU controls. This is local offscreen correctness evidence, not GPU filter execution, a performance gain, physical UI qualification or six-target conformance. Layered PDN/PAINT/PSD/PSB/ORA copies explicitly refuse nonempty stacks, including disabled entries, before output. Named flat-copy losses and browser snapshot accounting are integrated with separately captured policies.

Evaluate source-local filters before layer opacity and root composition. Add typed evaluation to CPU flat/region rendering, display tiles, thumbnails, clipboard/merged sampling, print and flat exports. Raw-layer sampling remains explicitly raw; merged/visible sampling uses evaluated appearances. Layered exports must preserve supported stacks or require an explicit bake/omission conversion with a real preview; preserving writers must refuse unsupported stacks.

Extend content cache keys with ordered semantic descriptors, profile version, mask dependencies, source default/dimensions and required source region versions. Parameter/enable/reorder edits invalidate appearances even when no raster tile changes. Compute input dependency and output damage through the stack. Pointwise first nodes have no halo; Box now supplies the regional halo/workspace contract; other spatial/global nodes still require their own contracts.

Placement-only reuse is permitted for these source-local point filters. Position-dependent materials, transformed masks, global statistics, backdrop-dependent styles and resampling require additional keys/bounds. Sharing an appearance does not authorize presenting a stale session/generation/request. Bound caches, queued jobs, scratch and upload copies under caller-owned realms; eviction cannot destroy committed descriptors/source.

PDN/PAINT/PSD/PSB/ORA preserving writers reject effect-bearing documents before pixel work: a rendered ORA merged preview does not preserve an editable stack in its raw layers. Flat and specialty copies now need a separately captured Bake policy, bound to the frozen document, stack digest and target operation. They return `filters-baked`, including for disabled/hidden intentions, while keeping the original project. Source-metadata Omit never authorizes dropping filters. DDS/MSP protocol v4 preserves its point/mask contract. Protocol 5 adds PCND5 spatial inventory/workspace binding and passes local codec/backend controls; its MAIN DDS export5 actual UI and fresh replay proof pass; MAIN MSP export5 actual UI also passes finite consent/replay checks. Historical actual point Worker/UI flows verify pixels and refusal/revocation. Raw-layer sampling or copy remains an explicitly named source-pixel mode.

GPU presentation may consume CPU-evaluated tiles initially. Never bypass active filters in a GPU fast path, picker, preview or export because the backend has no matching kernel. CPU fallback preserves correctness; unsupported resource sizes yield a typed failure rather than source-only rendering. Do not import Palette's whole-layer clones or fixed LRU limits as PaintCraft's resource policy.

Gate: full versus regional/tile-split equality, negative origins, finite source edge versus tile edge, implicit defaults, mask-once, distant invalidation for future global nodes, cold tile restoration, canceled/stale jobs, CPU/GPU-presented sample parity and export/print/clipboard consistency. Retain [ROI-01 through ROI-07](../temp/paintcraft-implementation-spec-v5-hyperfine/docs/architecture/EFFECT_REGIONS.md#tests).

### 4. Studio, native/web hosts and automation

The full App-suite and original browser-UI counts below describe the initial
two-filter checkpoint, before persistent masks. The subsequent mask checkpoint
adds native/browser host checks, actual fractional-mask UI presentation and
PCND4 reopen; its current scope is recorded in [FILTER_MASKS.md](FILTER_MASKS.md).

Desktop Studio exposes the shared presenter with seven-language labels. Private parameter previews use the admitted renderer, and captured edits pass through the normal FIFO. Eleven host groups cover real controls, live preview pixels, all stack actions, Apply/Cancel/Undo, retained metadata-limit failures with retry, stale ownership, cancellation during preparation and canvas navigation. The full native App suite passes (733 tests; 29 existing ignored), with strict checks. Browser controls use the same presenter, captured-edit guard and private Apply/Cancel semantics. Ten host groups cover lifecycle, compact snapshot boundaries and regional-texture cancellation; the full Web suite passes (378 tests; three existing ignored), with native and Wasm strict checks.

The separate persistent browser render Worker passes 16 actual Chrome controls against independent PCND fixtures and literal pixel expectations, including retained ICC and resolution-only attachments. Fifteen actual browser UI checks cover private parameter previews and displayed screenshot pixels, byte-identical source after Cancel, screenshot-measured Ctrl/Cmd zoom and middle-button panning while parameters stay open, preview-off Apply, background Undo/Redo, direct enable/reorder with noncommutative pixels, editing existing parameters without changing filter identity, and fresh application reopen/save. Independent PCND decoding checks filter UUIDs/order and raw pixels; fresh Save is byte-identical. These do not establish physical-device qualification or all remaining interactive gates. Parameter and committed filtered-view rendering leave the UI thread; initial bounded source serialization/hash/copy and ordinary tool previews remain synchronous. One synchronous metadata stage or output patch can delay cancellation; no maximum frame-latency claim follows.

Three additional actual browser checks exercise a valid compact-boundary project: Worker preview admission, real metadata-limit Apply/retry with the parameter window retained, and byte-identical source/profile after Cancel and fresh reopen. The reader allowance includes the published attachment header as well as the profile payload; the natural 8 MiB ceiling is unchanged. Host regressions cover late base frames over brush/text/IME/floating/paste previews, restoring an honest cached base after disjoint-view cancellation, and refusing stale private textures during immediate filter-dialog capture. Worker failure tests quarantine uncertain execution and settle queued edits; successful close uses an actual frozen acknowledgement rather than a fabricated receipt. Known completed typed refusals remain reusable.

Native sampled/pen/resource/move previews also enter the staged renderer. Host tests compare all four plus ordinary brush/shape/text with committed filtered appearance, opacity/backdrop composition, cancellation, stale completions and Undo, in synchronous and real worker modes. Browser IME preedit uses the actual text rasterizer and document compositor; private composition, real framework edit batches, texture changes and frontend ownership have host coverage. Physical IME/device qualification remains separate work.

Studio gets an ordered stack under each layer: add, edit, enable, reorder, duplicate and delete. Brightness/Contrast uses a private parameter draft, live preview and Apply/Cancel. Apply adds/edits the descriptor once; it never replaces the layer source with preview pixels. Freeze session, layer, document base, instance ID and draft generation; tab/layer/tool changes, cancellation and stale source retire the candidate without touching newer work.

Use the shared action outbox and captured host ownership across desktop and web; do not port Palette's desktop worker/channel code into browser hosts. A reduced preview must disclose its scale and cannot serve as exact full-resolution output qualification. An admitted actual visible preview precedes any destructive bake/export decision. Settings/language controls share existing localization and font infrastructure.

CLI/MCP expose typed stack operations and descriptor inspection with expected-state checks, atomic batches and the same validation/admission. File import/export, desktop, browser and automation use the same CPU appearance policy. Browser PCND save/reopen preserves editable stacks; explicit flat copies bake the evaluated view while retaining the project and its source-metadata privacy authority.

Gate: real native and production-browser add/edit/reorder/Cancel/Apply/Undo/save/reopen flows, actual CLI and stdio MCP exact descriptor/pixel comparisons, source unchanged after preview/export, default unsupported-preserving-export refusal and explicit conversion consent. Do not advertise UI, web or automation parity from core tests alone.

### 5. Grow the catalog through declared contracts

Add neighborhood filters only with halo/border/scratch/dirty-region proofs; global-analysis filters need immutable full-dependency analysis objects; geometric filters need mapped footprints and interpolation support. Masks, effect styles/shared lighting, generators, imported LUTs/patterns and plugins each require bounded payloads and distinct semantics. Continue from the author's [15-category inventory](../temp/IMPLEMENTED_FUNCTIONS.md#12-complete-implemented-filter-catalog), qualifying each family rather than claiming 132 integrations at once.

The ordered stack now has six implemented point profiles, fixed per-filter
masks, editable history/persistence and consistent native/web/CLI appearance.
The [mask qualification](FILTER_MASKS.md) retains its historical two-kind scope.
For the four additions, Commands has 21 passing tests; actual CLI/stdio-MCP has
five groups plus 61 schema controls over 32 genuine receipts and 43 legacy
regressions. The full Formats suite passes 769 tests. Renderer qualification
passes 108 tests, including 36 required Metal controls. The production Chrome
probe passes 305 persistent Worker cases plus 32 DDS/MSP protocol 4 export cases;
nine positive artifacts independently decode to exact fixture pixels. These
controls preserve explicit baking and source-metadata privacy authority.

Four actual Chrome Studio flows pass for the added kinds, with seven evidence
records per flow: Add, numeric parameter change for three profiles, Cancel, Apply,
exact Undo/Redo and fresh reopen/save. The independent red/green source makes
every default causal, confirmed by exact two-color screenshot markers at the
same canvas geometry. Changed Threshold's uniform black has exact Worker-plane
evidence only. Apply legitimately promotes its admitted private texture; fresh
reopen separately verifies strict committed BASE source/state/ROI binding.
These runs do not newly qualify existing-instance Edit, duplicate or preview-off.
Calibration and failed attempts remain separate from the four final passes.
The full Node suite passes 185 tests and independent JS oracle controls pass 63;
raw Worker, host and visible UI evidence are distinct.
[Point profiles](POINT_FILTER_PROFILES.md#integration-and-qualification) record
that distinction and the immutable production evidence. Mask painting/linking,
layer masks, other spatial/global effects, advanced styles and the rest of the pinned
132-entry source inventory remain required roadmap work.

## First spatial profile: Box Blur

Box Blur is integrated with radius 0–32 (new-draft default 2), true finite-source
CLAMP and exact premultiplied neighborhood sums. It can change alpha and uses
once-per-stage spatial coverage against the incoming prefix. The six point
contracts remain alpha-preserving. This independent policy differs from pinned
Palette per-axis rounding; it is not a direct source port or parity claim.

The [regional integration](SPATIAL_FILTER_INTEGRATION.md) covers reverse halos,
implicit defaults, ordered prefix buffers, shared workspace/work, CPU-before-LOD
and CPU-evaluated GPU uploads. PCND5, batch v5, inspect v4 and result/envelope4 use
conditional spatial semantics; old closed profiles and PCND1–4 bytes remain
unchanged. Layered foreign preserving copies still refuse intentions, while
explicit captured Bake authorizes evaluated flat/specialty copies separately
from metadata Omit.

The model, Effects, document/history, PCND5, regional renderer with required
Metal, typed CLI/stdio profiles and Native/Web/Worker controls are qualified
locally. Native actual Metal readback, PCND5 Save/reopen and single-layer
Flatten/Undo pass. [Current counts and scope](SPATIAL_FILTER_INTEGRATION.md#current-qualification)
record these separately from actual production Worker3 (17 sources/104 finite
controls), seven scoped Box Studio checks and MAIN DDS export5 UI plus nine
fresh replays with independent decoded pixels. MAIN MSP export5 actual UI also passes finite consent/replay checks with
independently decoded exact 2×1 opaque-white pixels. Existing Edit/duplicate/preview-off are not newly qualified by
the Box Studio run. Higher catalog entries, layer masks, styles, Gaussian/global
families and broad performance/compatibility remain open.

## Attribution and copying policy

The pinned [LICENSE](https://github.com/spiffyfitz/palette/blob/aefc84fe9233e66344147078de866860326d8cfa/LICENSE) is MIT, copyright © 2026 Palette contributors. For copied/adapted code or substantial documentation, preserve its copyright and full permission/warranty notice with the material and in distributed third-party notices; record repository, commit, source path/blob and modifications. Audit any separately licensed assets/dependencies before adopting them. A comment or GitHub link alone is not the full notice required for copied substantial portions.

Prefer small attributed algorithm/interaction adaptations over importing the application or its dependency stack. Independently implement PaintCraft's persistent types, admission, typed errors, concurrency guards, canonical formats and ROI contracts. The first scalar kernels are independently implemented from the documented mathematical policy. No Palette source or dependency stack has been copied into product code. Core test results qualify these kernels, not Palette or the remaining integration.
