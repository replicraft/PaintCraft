# Editable filters

The engine, CLI and MCP support an ordered filter stack on each raster layer.
The seven algorithms are Invert, Brightness/Contrast, Black & White, Threshold,
Posterize, Solarize and Box Blur. All preserve editable source pixels; the six
point profiles preserve alpha, while Box Blur evaluates a premultiplied
neighborhood and can change alpha. They participate in one-step Undo/Redo and remain editable after
saving and reopening a PCND project. Desktop and browser Studio expose stack
editing and per-filter selection-derived masks. Mask painting/linking and the
remaining algorithm catalog are separate work.

## Studio

Choose **Interface**, switch to **Studio**, and open your project there. Select
a layer in **Layers** to add, edit, enable, reorder, duplicate or remove its
filters. Parameters preview privately: **Apply** records one undoable edit,
while **Cancel** leaves the document unchanged. A failed Apply retains the
parameters for retry or cancellation. Each filter's actions menu offers **Use
Selection**, **Enable filter mask** and **Remove mask**. An enabled saved mask
limits that filter; a missing or disabled mask affects the whole layer. The
parameter dialog describes the current saved-mask state. Capturing an active
empty selection stores zero coverage, not a whole-layer mask.

The parameter window allows middle-button canvas panning and Ctrl/Cmd-scroll
zooming. Save as PCND to keep the stack editable. Exporting another image format
requires the separate baking consent described below.

Browser parameter previews and committed filtered views use a persistent
background Worker with one frozen source and bounded latest-request scheduling.
Results must match the exact owner, document, draft, parameters and viewport.
The browser editor retains its existing 4-megapixel canvas limit. Initial source
serialization, hashing and bridge copying remain synchronous and bounded by a
64 MiB wire limit; ordinary brush/text/floating previews also retain their
existing synchronous renderer. One output patch or metadata stage can delay
Worker cancellation, so this is not a strict frame-latency qualification.

Each layer supports up to 256 instances, including disabled instances. IDs,
order, enabled state and parameters are persistent document meaning. Filter
operations change metadata rather than rewriting source tiles. A metadata-limit
failure publishes neither edits nor aliases and leaves history unchanged.

## Operations

The six stack operations below use batch v2 except `filter.add` and
`filter.update` carrying one of the four new descriptors, which require batch v4.
Box Blur Add/Update requires batch v5. Mask operations require batch v3. Batch
v5 accepts all these operations; closed batch v2/v3/v4 grammars remain unchanged. Scalar enable/reorder/remove/duplicate
operations still require only v2, even when their target stores a newer kind.
Every operation takes `layer_ref`, using the existing `{ "id": "UUID" }` or
batch-local `{ "ref": "alias" }` form.

Complete Levels Add/Update uses the separate `paintcraft.batch/catalog-v1`
envelope. It requires all five integer fields: `input_black`, `input_white`,
`gamma_hundredths`, `output_black` and `output_white`, with kind
`levels-rgb-v1`. A submitted batch must include `required_filters`: sorted,
unique numerical profile IDs exactly matching its Add/Update payloads, including
legacy profiles in a mixed batch. Scalar operations do not infer requirements
from their target. Old batch/v1 through v5 remain closed.

Discover the immutable resolved bundle with `paintcraft-cli schema catalog`,
then request `schema batch-catalog`, `result-catalog`, `file-command-catalog`,
`inspect-catalog` or `mcp-inspect-catalog`. Catalog result/inspection requirements
describe every stored profile when Levels is retained, including disabled
instances; they differ from the operation inventory above and PCND6's transport
profile/codec pairs. Removing the last Levels restores the applicable older
result and project profile. See the [catalog contract](architecture/PCND_FILTER_CATALOG.md).

| Operation | Additional fields |
|---|---|
| `filter.add` | `ref`, `descriptor`; optional `enabled` (default true) and `to_index` (default end) |
| `filter.update` | `filter_ref`, complete replacement `descriptor` |
| `filter.set_enabled` | `filter_ref`, `enabled` |
| `filter.reorder` | `filter_ref`, final zero-based `to_index` |
| `filter.remove` | `filter_ref` |
| `filter.duplicate` | `filter_ref`, new `ref`; optional `to_index` (default immediately after source) |

Filter references use the same ID/ref object syntax, with a separate alias
namespace qualified by the owning layer. Aliases exist only within their batch;
later batches use the instance IDs returned by inspection. Removed aliases
cannot be reused within the same batch. Unknown fields, references, algorithms,
out-of-range indices and invalid parameters are rejected atomically.

The original descriptors are `{ "kind": "invert-rgb-v1" }` and:

```json
{
    "kind": "brightness-contrast-v1",
    "brightness": 10,
    "contrast": 0
}
```

Brightness is an integer in -100 through 100; contrast is -99 through 99. These
are hundredths, not channel bytes. The [versioned arithmetic](architecture/PALETTE_INTEGRATION.md#persistent-model-and-arithmetic)
works on encoded RGB and preserves alpha, including transforming hidden RGB.
Retained original ICC profiles are not activated by filters.

The additional descriptors require batch v4 only for Add/Update:

| Kind | Required parameter | Inclusive integer range | New-draft default |
|---|---|---|---|
| `black-white-rgb-v1` | `strength` | 0–100 | 100 |
| `threshold-luma-v1` | `level` | 0–255 | 128 |
| `posterize-rgb-v1` | `levels` | 2–32 | 6 |
| `solarize-rgb-v1` | none | fixed threshold 128 | none |

For example, `{ "kind": "posterize-rgb-v1", "levels": 6 }` is a complete
descriptor. Required fields cannot be omitted to request defaults. Unknown fields,
versions, fractional values and invalid domains are refused. All six kernels use
encoded RGB, preserve alpha and hidden source RGB, have source-local halo zero,
and apply an instance mask once after rounding that stage's candidate. Exact
[point-profile arithmetic](architecture/POINT_FILTER_PROFILES.md) defines unrounded
Threshold luma, Posterize's two rounding steps and Black & White strength zero
identity. These are independent PaintCraft byte contracts, not Photoshop or
Palette floating-point byte-parity claims.

Box Blur uses `{ "kind": "box-blur-v1", "radius": 2 }`, with required integer
radius 0–32 and new-draft default 2. Add/Update requires batch v5, even at radius
zero. Positive radius uses a fixed square with true finite-source CLAMP, not
tile-edge clamping; radius zero preserves every RGBA byte. Its candidate can
change alpha and uses the separate once-per-stage premultiplied coverage rule.
See the [exact spatial contract](architecture/SPATIAL_FILTER_INTEGRATION.md).
Stored disabled and radius-zero intentions still require spatial support.

## Persistent mask operations

Masks are immutable source-local A8 fields, independent of the live selection.
The six point filters preserve alpha and mix candidate RGB once per instance,
including hidden RGB. Box Blur instead mixes incoming and candidate RGBA once
with the alpha-changing spatial rule; coverage never limits its input halo. No
mask or a disabled mask means full coverage; an enabled zero mask means no
visual effect. Descriptor updates, instance enablement, reorder and duplication
preserve the mask. See the [mask contract](architecture/FILTER_MASKS.md).

| Batch v3 operation | Additional fields |
|---|---|
| `filter.mask.set` | `filter_ref`, `mask`; optional `enabled` (default true) |
| `filter.mask.from_selection` | `filter_ref`; optional `enabled` (default true) |
| `filter.mask.set_enabled` | `filter_ref`, required `enabled` |
| `filter.mask.remove` | `filter_ref` |

`set` takes a numeric `mask` object with `domain: {x,y,width,height}` and
`bands: [{y0,y1,runs:[{x0,x1,coverage}]}]`. Band/run coordinates are absolute
source-local endpoints in the finite domain; coverage is an integer 0–255.
Input zero runs and adjacent equal records are canonicalized. The positive finite
domain need not equal the raster bounds: negative or smaller domains and latent
off-layer coverage are retained. Outside coverage is zero; evaluation remains
confined to the layer source extent. Counts, coordinates, construction work and
actual allocation capacity are bounded before publication.

`from_selection` freezes the staged selection at that operation, converting
from document to source-local coordinates. Inactive selection is an error;
active empty or disjoint selection yields a stored zero mask. Rectangular,
arbitrary binary and inverted selections are supported. Later selection changes
cannot alter the captured field. Preparation captures admitted coverage once;
replay reuses it. Removing an absent mask, or setting its existing enabled value,
is a no-op after valid reference resolution. Enabling a nonexistent mask is an
error. Earlier closed batch v1/v2 schemas refuse all four mask tags.

Source crop/orthogonal transforms map coverage with the raster. Moving selected
pixels keeps the spatial mask fixed; an origin rebase translates its local view
without clipping its declared domain or latent coverage. These operations are
atomic with the source edit and Undo/Redo.

## MCP example

Get a layer ID from `inspect_document`, replace the example UUID, then call
`apply_operations` with:

```json
{
    "operations": [
        {
            "op": "filter.add",
            "layer_ref": { "id": "00000000-0000-0000-0000-000000000001" },
            "ref": "tone",
            "descriptor": {
                "kind": "brightness-contrast-v1",
                "brightness": 10,
                "contrast": 0
            }
        }
    ]
}
```

To capture a selection on a new instance in the same atomic operation, use a
rectangle appropriate for the open canvas and the same layer UUID:

```json
{
    "operations": [
        {"op":"selection.set_rect","rect":{"x":0,"y":0,"width":1,"height":1}},
        {
            "op":"filter.add",
            "layer_ref":{"id":"00000000-0000-0000-0000-000000000001"},
            "ref":"masked",
            "descriptor":{"kind":"invert-rgb-v1"}
        },
        {
            "op":"filter.mask.from_selection",
            "layer_ref":{"id":"00000000-0000-0000-0000-000000000001"},
            "filter_ref":{"ref":"masked"}
        },
        {"op":"selection.clear"}
    ]
}
```

The server uses batch v3 for this request. The saved field reflects the selection
at the capture operation, even though the batch clears the live selection
before publication. The whole successful batch is one Undo step; any failure
publishes none of it. Inspection exposes the new stable instance ID; aliases
are not carried into a later batch.

The server captures the current base ticket. `inspect_document` includes an
ordered `filters` array only for layers with a nonempty stack. A duplicate gets
a new ID; changing parameters preserves the existing ID.

## CLI and result contracts

`apply` accepts these operations through an ordinary atomic batch file. MCP
`apply_operations` supplies the current ticket and selects the lowest required
grammar: v5 for Box Blur Add/Update, v4 for the four newer point kinds, otherwise
v3 when mask tags are present, otherwise v2. `get_batch_schema` returns the closed v5 grammar. Retrieve the closed profiles:

```sh
paintcraft-cli schema batch-v2
paintcraft-cli schema batch-v3
paintcraft-cli schema batch-v4
paintcraft-cli schema batch-v5
paintcraft-cli schema result-v2
paintcraft-cli schema result-v3
paintcraft-cli schema result-v4
paintcraft-cli schema file-command-v2
paintcraft-cli schema file-command-v3
paintcraft-cli schema file-command-v4
paintcraft-cli schema inspect-v2
paintcraft-cli schema inspect-v3
paintcraft-cli schema inspect-v4
paintcraft-cli schema mcp-inspect-v2
paintcraft-cli schema mcp-inspect-v3
paintcraft-cli schema mcp-inspect-v4
```

Output selects the lowest necessary profile from the **final document and
published aliases**, not merely the input batch version:

| Final result | Engine result | CLI envelope |
|---|---|---|
| Any stored Box Blur, including disabled/radius zero | `paintcraft.result/v4` | `paintcraft.file-command/v4` |
| No spatial intention, any stored mask, including zero/disabled | `paintcraft.result/v3` | `paintcraft.file-command/v3` |
| No spatial intentions or masks, nonempty new filter aliases | `paintcraft.result/v2` | `paintcraft.file-command/v2` |
| No spatial intentions, masks or new filter aliases | exact `paintcraft.result/v1` | exact `paintcraft.file-command/v1` |

The nonempty `filter_aliases` map contains `{layer_id,filter_id}` UUID pairs. It
is required in v2 and optional, but nonempty when present, in v3. Result v3 also
requires `document_features` in canonical order:
`["layer-point-filters.v1","layer-filter-masks.v1"]`, followed by
`"source-metadata.v1"` only when an original-source attachment exists. When no spatial intention is stored, a no-op on an already masked document
still uses v3. Removing the final mask in that point-only document returns to
v1 or v2 according to aliases, even when the input is batch v3 or v4. New
point algorithm kinds alone do not introduce a new result profile or feature.
Stored Box Blur does require result/envelope v4: features are
`layer-point-filters.v1`, optional `layer-filter-masks.v1` iff a mask is stored,
`layer-spatial-filters.v1`, then optional `source-metadata.v1`. Removing the last
spatial intention returns to the minimal v1/v2/v3 profile from the final document.
A batch v5 request alone never forces result v4. Closed legacy
result/envelope schemas are unchanged; error-only envelopes retain v1.

If any stored instance is Box Blur, including disabled/radius zero, CLI uses
`paintcraft.inspect/v4` and MCP uses distinct `paintcraft.mcp-inspect/v4`.
Otherwise, if any stored instance uses one of the four newer point kinds, including a disabled
instance, CLI inspection uses `paintcraft.inspect/v3` and MCP uses the distinct
`paintcraft.mcp-inspect/v3`. Otherwise, masked CLI inspection uses
`paintcraft.inspect/v2`; masked MCP uses `paintcraft.mcp-inspect/v2`. The CLI
includes its source assessment, document and ticket; MCP retains its dimensions,
dirty/history and layer projection. Their separate closed schemas must not be
interchanged. With only the original kinds and no masks, CLI inspection remains
v1 and MCP inspection retains its established unversioned output. Inspection v3
includes the same ordered `document_features` only when masks are stored; new
kinds alone do not add it. Mask summaries retain their existing bounded shape.

A stored mask adds only this bounded summary to its filter row:

| Summary field | Meaning |
|---|---|
| `schema` | `a8-bands/v1` |
| `enabled` | Boolean, independent of the instance's enabled state |
| `domain` | Signed `[x0,y0,x1,y1]` endpoints in source-local coordinates |
| `bands`, `runs` | Canonical record counts |
| `digest` | Lowercase 64-character SHA-256 of the canonical field |

Absent masks omit the member. Inspection never dumps coverage payloads or
confers editing/bake authority.

## Saving and rendering

A stored mask selects [PCND4](PCND_FORMAT.md#version-4-persistent-filter-masks),
including disabled and all-zero fields. Unmasked nonempty stacks use
[PCND3](PCND_FORMAT.md#version-3-live-point-filters), even when instances are
disabled; empty stacks retain exact PCND1/2 profiles. Unknown algorithm or mask
codec versions are refused rather than dropped, even for disabled instances.
The additive [algorithm registry](PCND_FORMAT.md#version-3-live-point-filters)
uses tags 3–6 for the new kinds within PCND3/4; no container version bump is
needed, and tags 1/2 retain their exact original parameter bytes. The unchanged 8 MiB metadata
ceiling includes mask records. Aggregate serialized counts are limited to
262,144 bands and 262,144 runs; shared host payloads count per serialized instance.
Default binary selections and retained A8 coverage share one 256 MiB resident
realm, with explicit caller realms available. History preserves complete masks
and deduplicates shared payload charges; masks remain resident when raster
history spills. These are charged-resource bounds, not process RSS limits.

CPU composition and GPU source uploads use the same point-filter appearance,
before layer opacity. Visible sampling, thumbnails and visible-image copies use
that appearance; explicitly raw layer sampling reads the original pixels.
GPU filter kernels, the remaining catalog and mask painting/linking remain work.
Layered foreign writers refuse these stacks.

## Exporting a rendered copy

Foreign image copies refuse nonempty stacks by default. Baking needs a separate
explicit choice, bound to the frozen document and destination format. It retains
the rendered appearance in the copy while the open project keeps its editable
stack and unchanged source pixels. Disabled and hidden filters also count as
editable information: their removal must be acknowledged even when they do not
change the exported pixels.

The CLI uses `--allow-loss filters-baked`; MCP uses `"filters-baked"` in the
request's `allow_losses` array. Other actual losses still need their own consent.
For example, a document with multiple layers may also need `flatten`. Retained
source metadata requires its independent `"source_metadata": "omit"` or
`--source-metadata omit` choice. Neither metadata omission nor a generated list
of every known loss can authorize baking.

Library callers capture `export::filter_bake::Policy` with explicit
`Intent::Bake`, then pass it through `FlatOptions::filter_bake` or the specialty
writer's `prepare_document_with_policies` API. `export::CopyPolicies` combines
the two independently captured policies. Metadata-only and legacy entry points
continue to refuse filters. Changing the stack, document state or target format
invalidates the captured bake policy, including any mask presence, enabled state
or coverage change. PCND saves preserve filters directly and
do not use this conversion policy.

Desktop and browser export dialogs start with baking unchecked. Changing the
choice retires the prepared copy; another download needs fresh consent. Actual
Chrome PNG/DDS/MSP flows verify refusal, revocation, exact exported pixels and
an unchanged project. Native controls, CLI/MCP exports and the installed JPEG XR
writer also have focused runtime coverage.

Desktop Studio stack editing has host coverage for private previews, actual
background preparation, Apply/Cancel/Undo, stale ownership, metadata-limit
failure and retry. Browser editing has captured-command dispatch and host
lifecycle coverage. The built persistent renderer passes 16 actual Chrome
controls with independent PCND fixtures and pixel expectations, including source
reuse, preview-off preparation, typed refusal and retirement acknowledgements,
including valid retained ICC and resolution-only attachments.
Actual browser UI coverage also verifies private parameter previews, Cancel,
canvas navigation, preview-off Apply, Undo/Redo, editing existing parameters,
direct enable/reorder, and a
fresh application reopening and saving the editable stack. Independent PCND
decoding checks filter identities/order and unchanged raw source pixels.
An actual compact-boundary project also verifies metadata-limit refusal and
retry with parameters retained, fenced Save while the draft is active, and
byte-identical Save/reopen after Cancel. Host controls prevent late committed
frames from replacing brush, text, IME, floating or paste previews, including
Cancel after a viewport change and immediate reopening of a filter dialog.

Current persistent-mask renderer, host, CLI/MCP, wire and production browser
qualification is summarized in [FILTER_MASKS](architecture/FILTER_MASKS.md).
Those completed browser results cover fixed saved masks on the two initial
algorithms. The six-kind extension has qualified Core/Effects/Document,
Commands, actual CLI/MCP and Native/Web host UI controls and strict checks.
Independent CLI/stdio-MCP controls exercise literal and fractional-mask pixels,
order, unchanged raw source rasters, exact result profiles, disabled-kind
inspection and PCND save/reopen. The full Formats suite passes 769 tests and
all 108 renderer tests pass, including 36 required offscreen Metal controls.
Actual production Chrome passes 305 render Worker and 32 DDS/MSP protocol 4
export cases; nine positive export files independently decode to exact fixture
pixels. Four actual Chrome UI flows now pass Add, Cancel, Apply, exact Undo/Redo
and fresh reopen/save for the new kinds, plus parameter changes for the three
numeric profiles. All four defaults have causal two-color screenshot markers;
changed Threshold's uniform output is an exact Worker-plane check only. Apply
can promote its admitted private texture; fresh reopen independently verifies
strict committed BASE binding to the saved source/state and ROI. These flows do
not newly qualify existing-instance Edit, duplicate or preview-off. Host checks,
raw Worker frames and visible UI execution remain separate evidence. The full
Node suite passes 185 tests and independent JS oracle controls pass 63. See the
[profile qualification scope](architecture/POINT_FILTER_PROFILES.md#integration-and-qualification).
Mask painting, layer styles and the rest of the pinned 132-entry source catalog
remain separate work; seven profiles do not establish catalog parity.

### Box Blur qualification

Core/Effects/Document, typed Commands, PCND5/recovery, actual CLI/stdio closed
profiles, regional rendering and required offscreen Metal are qualified locally.
Studio and Native controls cover private preview, Cancel/Apply, CPU sync/FIFO,
real Metal texture/readback, PCND5 save/reopen and single-layer Flatten/Undo.
Existing lifecycle regressions, Web host/Worker, protocol 5 codec/backend,
strict checks and all seven locales pass. The [current spatial qualification
record](architecture/SPATIAL_FILTER_INTEGRATION.md#current-qualification) gives
counts and exact boundaries.

Actual production Chrome passes Worker3 over 17 sources/104 finite controls and
seven Box Studio checks: Add, default radius 2/change to 1 with causal screenshots,
Cancel, exact Apply, Undo/Redo and fresh PCND5 reopen/save. Apply may promote the
admitted private texture; fresh reopen independently binds committed BASE. This
does not newly qualify existing Edit, duplicate or preview-off. MAIN DDS export5
capture/revoke/recheck/download passes, with nine fresh Worker replays (eight
negative) and independent Pillow pixels. MAIN MSP export5 also passes those finite consent/replay checks, with
independent exact 2×1 opaque-white pixels. The frozen 27-asset production receipts and current library/integration
regression counts are recorded in the [spatial qualification](architecture/SPATIAL_FILTER_INTEGRATION.md#production-browser-qualification).
Previous six-point browser results retain their original scope. Higher catalog
entries, layer masks and styles remain open.
