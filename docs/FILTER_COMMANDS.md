# Editable point filters

The engine, CLI and MCP support an ordered filter stack on each raster layer.
The first algorithms are Invert and Brightness/Contrast. They preserve source
pixels and alpha, participate in one-step Undo/Redo, and remain editable after
saving and reopening a PCND project. Desktop Studio exposes stack editing;
browser Studio controls and selection-derived filter masks remain work.

## Desktop Studio

Choose **Interface**, switch to **Studio**, and open your project there. Select
a layer in **Layers** to add, edit, enable, reorder, duplicate or remove its
filters. Parameters preview privately: **Apply** records one undoable edit,
while **Cancel** leaves the document unchanged. A failed Apply retains the
parameters for retry or cancellation. Filters currently affect the whole layer.

The parameter window allows middle-button canvas panning and Ctrl/Cmd-scroll
zooming. Save as PCND to keep the stack editable. Exporting another image format
requires the separate baking consent described below.

Each layer supports up to 256 instances, including disabled instances. IDs,
order, enabled state and parameters are persistent document meaning. Filter
operations change metadata rather than rewriting source tiles. A metadata-limit
failure publishes neither edits nor aliases and leaves history unchanged.

## Operations

Use batch v2. Every operation takes `layer_ref`, using the existing `{ "id":
"UUID" }` or batch-local `{ "ref": "alias" }` form.

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

Descriptors are either `{ "kind": "invert-rgb-v1" }` or:

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

The server captures the current base ticket. `inspect_document` includes an
ordered `filters` array only for layers with a nonempty stack. A duplicate gets
a new ID; changing parameters preserves the existing ID.

## CLI and result contracts

`apply` accepts these operations through the ordinary atomic batch file. Retrieve
the complete command and result schemas with:

```sh
paintcraft-cli schema batch-v2
paintcraft-cli schema result-v2
paintcraft-cli schema file-command-v2
```

A result containing new filter aliases uses `paintcraft.result/v2`. Its required
`filter_aliases` object maps each alias to `{ "layer_id": "UUID", "filter_id":
"UUID" }`. The CLI wraps that result in `paintcraft.file-command/v2`. Results
without filter aliases retain the exact v1 profiles, including no-ops, ordinary
edits, toggles and removals. The original closed v1 schemas remain unchanged.

## Saving and rendering

Nonempty stacks use [PCND3](PCND_FORMAT.md#version-3-live-point-filters), even if
all instances are disabled. Documents with empty stacks retain the existing
PCND1/2 encodings. Unknown algorithm versions are refused rather than dropped.
Cold history preserves the exact stack while its raster tiles are compressed
or spilled.

CPU composition and GPU source uploads use the same point-filter appearance,
before layer opacity. Visible sampling, thumbnails and visible-image copies use
that appearance; explicitly raw layer sampling reads the original pixels.
GPU filter kernels, the remaining catalog, masks and browser Studio controls remain
work. Layered foreign writers refuse these stacks.

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
invalidates the captured bake policy. PCND saves preserve filters directly and
do not use this conversion policy.

Desktop and browser export dialogs start with baking unchecked. Changing the
choice retires the prepared copy; another download needs fresh consent. Actual
Chrome PNG/DDS/MSP flows verify refusal, revocation, exact exported pixels and
an unchanged project. Native controls, CLI/MCP exports and the installed JPEG XR
writer also have focused runtime coverage.

Desktop Studio stack editing has host coverage for private previews, actual
background preparation, Apply/Cancel/Undo, stale ownership, metadata-limit
failure and retry. Browser editing has captured-command dispatch coverage;
its interactive controls and background preview service remain separate work.
