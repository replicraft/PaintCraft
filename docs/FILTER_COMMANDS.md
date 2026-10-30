# Editable point filters

The engine, CLI and MCP support an ordered filter stack on each raster layer.
The first algorithms are Invert and Brightness/Contrast. They preserve source
pixels and alpha, participate in one-step Undo/Redo, and remain editable after
saving and reopening a PCND project. Studio controls and selection-derived
filter masks are not available yet.

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
GPU filter kernels, the remaining catalog, masks and Studio controls remain
work. Layered foreign writers refuse these stacks. Complete flat-copy bake
consent and browser worker binding are still part of the integration plan;
source-metadata omission never grants permission to discard filter settings.
