# Painting with MCP

PaintCraft includes a headless [Model Context Protocol](https://modelcontextprotocol.io/) server built with [rmcp](https://github.com/modelcontextprotocol/rust-sdk). It creates and edits its own document; it does not control an open desktop window.

Build the CLI:

```sh
cargo build --release -p paintcraft-cli
```

Configure your MCP client with the absolute path to that binary:

```json
{
    "mcpServers": {
        "paintcraft": {
            "command": "/absolute/path/to/paintcraft-cli",
            "args": ["mcp"]
        }
    }
}
```

The server uses stdio. Its stdout contains only MCP messages. It runs with your account's local file access; connect clients you trust.

Available tools:

- `create_canvas`: create a document with a white or specified background.
- `open_project`: open a layered PCND project.
- `import_image`: import a local picture into a new unsaved project, with explicit conversion-loss consent.
- `inspect_document`: inspect dimensions, layer IDs and history.
- `paint_stroke`: paint with the engine's brushes.
- `draw_shape`: draw filled or outlined shapes.
- `get_batch_schema`: discover all supported engine operations.
- `apply_operations`: apply layers, fills, text, selections, transforms and other operations as one atomic batch.
- `flatten_image`: combine the visible canvas into one layer, with Undo restoring the source stack.
- `history`: undo or redo.
- `preview`: return a PNG preview, at most 256 pixels per side.
- `save_project`: save a layered PCND project.
- `export_image`: export an image using its filename extension.
- `export_icons`: export a fresh multi-size ICO/CUR asset set with explicit sizes, PNG/bitmap encodings and cursor hotspots.
- `inspect_icns`: list ordered ICNS entries, logical sizes, Retina densities and import qualification without changing the painting.
- `export_icns`: export a fresh ICNS set using explicit qualified PNG tags and independently fitted entry sizes.
- `inspect_palette`: inspect an ASE palette resource without creating or editing a painting.
- `export_palette`: write a fresh ASE palette copy while retaining groups, names, color models, swatch types and precise channel values.
- `inspect_shapes`: inspect a CSH custom shape library, including exact bounds and ordered path records.
- `export_shapes`: write a fresh CSH resource copy without editing or adopting the open document.

Palette tools work without an open document. They never edit the painting,
history, dirty flag or project save binding. For example:

```json
{"name":"inspect_palette","arguments":{"path":"/absolute/path/colors.ase"}}
{"name":"export_palette","arguments":{"input":"/absolute/path/colors.ase","path":"/absolute/path/copy.ase","overwrite":false}}
```

Use `expect_input_sha256` to require the exact source digest returned by
inspection. Output must be a distinct `.ase` file; filesystem aliases of the
palette and the painting's protected sources are refused. Inspection's
`channels_f32_be` fields contain hexadecimal IEEE754 f32 bits, preserving the
source values without RGB conversion. The equivalent CLI commands are:

```sh
paintcraft-cli palette inspect colors.ase --json
paintcraft-cli palette export colors.ase -o copy.ase
paintcraft-cli schema palette-inspection
```

Studio desktop and browser expose palette import/export through File > Palettes
and the Colors window's Palettes button. Picking a normalized RGB or gray swatch
copies its rounded 8-bit sRGB color into the selected primary/secondary tool
color. Palette values stay unchanged. CMYK/LAB display conversion and original
Adobe application qualification remain separate work. Image Open/Import does not
treat palette assets as pictures.

## Custom shape resources

`inspect_shapes` and `export_shapes` exchange Photoshop custom shape libraries
as Studio resources. They work without a painting and never change document
pixels, layers, history, dirty state or the project save binding. Inspection
returns `paintcraft.shape-inspection/v1`, including:

- Shape names and original IDs, retaining duplicate IDs in their original order.
- Original unsigned `bounds_tlbr` values, including the origin.
- Ordered subpath, knot, fill-rule, initial-fill and clipboard records.
- Exact signed normalized 8.24 `points_i32` coordinates in
  `[x0, y0, x1, y1, x2, y2]` order, without floating-point or pixel rounding.
- Open/closed and linked/unlinked flags, knot counts, nullable path operations
  (`exclude`, `combine`, `subtract`, `intersect`), and distinct legacy even/odd,
  even/odd and nonzero path-fill flags. An absent
  source operation is explicit JSON `null`.

Clipboard `values_i32` retain `[top, left, bottom, right, resolution]` in signed
8.24 units. `source_sha256` identifies the exact acquired bytes; pass it as
`expect_input_sha256` to guard a later inspection or export.

```json
{"name":"inspect_shapes","arguments":{"path":"/absolute/path/shapes.csh"}}
{"name":"export_shapes","arguments":{"input":"/absolute/path/shapes.csh","path":"/absolute/path/copy.csh","overwrite":false}}
```

The equivalent CLI commands and complete typed JSON schema are:

```sh
paintcraft-cli shape inspect shapes.csh --json
paintcraft-cli shape export shapes.csh -o copy.csh
paintcraft-cli schema shape-inspection
```

Only the Studio profile admits these resource operations. The bounded profile
supports CSH version 2 / shape version 1, up to 4 MiB, 256 shapes and 65,536
ordered records, with null-free ASCII IDs and validated unsigned bounds and
reserved fields. Unsupported flags, record kinds and inconsistent topology are
refused. Fresh exports retain the admitted resource semantics; name
padding and terminators may be canonicalized. Output requires a distinct `.csh`
path, and existing files require explicit overwrite. Filesystem aliases of the
resource and the painting's protected sources are refused. Export receipts
identify the resource, never acknowledge a document save or adopt its lineage.
These operations do not open CSH as an image or place shapes into the painting.
The owned fixtures and upstream-reader checks do not establish Adobe application
or cross-platform release qualification.

For ICO/CUR import, provide a zero-based `entry` in `import_image`; no entry is
selected automatically. For DCX, provide a zero-based `page` instead. The two
fields cannot be combined. Import reports losses that must be acknowledged in
`allow_losses`: unselected icon entries use `icon-other-entries-not-imported`,
palette expansion uses `icon-palette-expanded`, and converting a cursor into a
picture uses `cursor-hotspot-not-retained-in-flat-document`. The original source
remains unchanged. CLI import uses the corresponding `--entry` or `--page` flags.

`import_image` also converts the supported RGB8 Normal raster-layer subset of
PSD/PSB into an unsaved layered project. Names, positions, visibility, opacity
and alpha are retained. Acknowledge each reported loss in `allow_losses`, such
as `psd-metadata-ignored`, `icc-profile-ignored` or
`psd-empty-layer-extent-normalized`. Masks, groups, text, effects, clipping,
non-Normal blends and high-depth files are refused. Save the converted copy as
PCND; importing never permits replacing the original Photoshop source or its
filesystem aliases. The equivalent CLI command is:

```sh
paintcraft-cli import drawing.psd -o converted.pcnd --allow-loss psd-metadata-ignored
```

Add any other reported loss codes explicitly. CLI Classic profiles refuse PSD
and PSB imports; the default Studio profile supports this conversion.

For SVG/SVGZ, `import_image` requires an explicit `svg` object and
`svg-rasterized` consent. This creates an unsaved raster copy, not editable
vectors. For example, pass these arguments to the tool:

```json
{
    "path": "drawing.svg",
    "svg": { "width": 1000, "height": 486, "dpi": 96, "fit": "contain" },
    "allow_losses": ["svg-rasterized"]
}
```

`width` and `height` are required; DPI defaults to 96 and must be between 1 and
2400. `contain` preserves aspect ratio on transparent padding; `stretch` scales
both axes independently. `svg`, `page` and `entry` cannot be combined. SVG input
is limited to 8 MiB, dimensions to 16,384 per side and output to 4,194,304 pixels.

Bundled Selawik regular/bold are available by default. Add explicitly chosen
local outline TTF/OTF/TTC files with `svg.font_files`, an array of paths. This
supports additional languages, such as the shared Noto Japanese font. Missing
glyphs refuse the import. Family fallback requires `svg-font-substituted` consent.
Font data is limited to 64 MiB and 64 faces, with at most 30 additional files.
Collection counts and name metadata are checked before loading; bitmap, SVG and
color font glyph tables need separate qualification and are currently refused.
Neither the original SVG nor supplied font files can be save/export destinations
through their filesystem aliases while that imported session remains active.

The corresponding CLI options are:

```sh
paintcraft-cli import drawing.svg -o converted.pcnd \
    --svg-width 1000 --svg-height 486 --svg-dpi 96 --svg-fit contain \
    --allow-loss svg-rasterized
```

Repeat `--svg-font /path/to/font.ttf` for additional fonts. Acknowledge any other
reported metadata or font-substitution losses explicitly. Classic CLI profiles
refuse SVG before reading its content. Studio desktop/browser Import provides
explicit dimensions, DPI and fitting controls, preview and loss consent. These
options do not enable ordinary SVG Open or SVG export.

`export_icons` takes `path`, an ordered `entries` array and optional `resampling`
(`lanczos3` by default, or `nearest`). Each entry has `width`, `height`, `encoding`
(`png` or `bitmap`) and, for CUR only, `hotspot: [x, y]`. Dimensions must be 1..256;
CUR hotspots are required for every entry and use that entry's pixel coordinates.
Each size is fitted independently from the original canvas, preserving its aspect
ratio on transparent padding. `allow_losses` must include `icon-resized` when
requested dimensions differ, plus `flatten` and `hidden-rgb` when applicable.
Fully transparent bitmap entries require choosing PNG instead. This is a fresh
export, not a preserving save of an imported multi-entry source. Export does not
change the current document, saved state or history.

The equivalent CLI command accepts repeated explicit entries:

```sh
paintcraft-cli export-icons painting.pcnd -o painting.ico \
    --entry 16:png --entry 32:bitmap --entry 256:png --allow-loss icon-resized
paintcraft-cli export-icons painting.pcnd -o pointer.cur \
    --entry 32:png:7:9 --allow-loss icon-resized
```

Use `WIDTHxHEIGHT` instead of a single size for rectangular entries. Existing
destinations require `--overwrite` / `overwrite: true`. CLI source digest guards
use `--expect-input-sha256`. Studio on desktop and web offers the same choices under File > Export icon or cursor.

PCX exports accept the optional `pcx_encoding` field: `rgb24` (default),
`indexed-exact` (refuse more than 256 colors), or `indexed-256` (explicitly allow
palette preparation). If colors actually change, include `palette-quantized` in
`allow_losses`; transparency separately requires `alpha-discarded`. Palette
options on another file format are rejected. Exact small palettes stay exact
even when `indexed-256` is requested. Export never changes the open document.

Coordinates are canvas pixels. Colors use `#RRGGBBAA`. Stroke and shape tools default to the top layer; `inspect_document` provides IDs for choosing another layer. Batch operations use the existing command schema, while the server supplies the current concurrency ticket.

Save explicitly before disconnecting: the session is held in memory and ends with the server process. Replacing unsaved work requires `discard_changes: true`. Existing output files require `overwrite: true`, except saving back to the opened project, which checks its captured SHA-256 digest and rejects external changes. Image imports and exports use the CLI Studio format-admission profile. Importing a picture never binds project saves to that source image. Image exports require explicit `allow_losses` acknowledgements when flattening or another conversion would discard information. Exporting does not mark a project saved.

A failed batch publishes no changes. Losing a connection after sending a mutation leaves its outcome uncertain; do not automatically retry it. This initial server does not provide durable request deduplication or recovery across process restarts.

## Imported brush resources

`inspect_brushes` reports ordered ABR preset names, native-static eligibility,
unsupported features and required spacing conversions without creating or
changing a painting. Inspection publishes `paintcraft.brush-inspection/v1`, with
a discoverable typed JSON schema, and admits at most 256 presets per library.
`adopt_brush` selects an ephemeral imported preset under
explicit `native-grid-v1` policy. It retains the acquired source and exact R8
tip; selection alone changes no document, history, dirty flag or save binding.
A failed adoption preserves the previous selected brush.

```json
{"name":"inspect_brushes","arguments":{"path":"/absolute/path/brushes.abr"}}
{"name":"adopt_brush","arguments":{"path":"/absolute/path/brushes.abr","preset":0,"policy":"native-grid-v1","allow_spacing_quantization":false}}
{"name":"paint_sampled_stroke","arguments":{"points":[[24,32],[48,48]],"color":"#204080FF","point_scale":1}}
```

Painting uses captured integer `[x,y]` points, a selected or topmost raster layer
and the existing atomic mask executor. A blank project gains its first transparent
canvas-sized layer and stroke in one Brush history entry. Undo removes both;
zero-opacity, empty or fully clipped coverage leaves no first layer or history
entry. `point_scale` may be 1 or 256; color alpha controls opacity once. A
`layer_id` can select an existing layer explicitly.

The native static policy preserves sample dimensions, uses Q8 translations,
integer Q32 path spacing and maximum coverage union. It is not Photoshop brush
parity. Higher-depth masks, computed tips, size/angle/roundness/flip overrides,
active dynamics, pressure behavior, texture/dual brushes and unknown fields are
refused, rather than approximated as stock brushes. Non-exact Q8 spacing requires
`allow_spacing_quantization: true`. See [sampled brush qualification](SAMPLED_BRUSHES.md)
for the exact policy and resource limits.

The MCP session protects selected and painted brush source identities from
save/export, including renamed files and hard-link aliases. It retains at most
sixteen distinct painted source resources in a document workspace; replacing the
document releases those workspace references. Source identity is acquired from
the same open file used for decoding. Existing destination identity errors fail
closed; nonregular destinations are not opened for identity comparison.

The equivalent CLI workflow is:

```sh
paintcraft-cli brush inspect brushes.abr --json
paintcraft-cli schema brush-inspection
paintcraft-cli brush paint drawing.pcnd --brush brushes.abr --preset 0 \
    --policy native-grid-v1 --points points.json --color '#204080FF' \
    -o painted.pcnd
```

`points.json` contains `[[24,32],[48,48]]` in this example. Use
`--allow-spacing-quantization` only after inspecting the reported conversion.
`--expect-brush-sha256` guards the brush bytes, and `--expect-input-sha256` guards
the project bytes. Studio is the supported asset profile; Classic and ordinary
image Open/Import do not treat ABR files as images. ABR export is unsupported.

## Historical MSP pictures

Explicit image import supports the bounded Windows Paint `.msp` monochrome
profile. It is separate from modern layered `.paint` and does not use ordinary
Open. Metadata consent is mandatory:

```json
{"name":"import_image","arguments":{"path":"/absolute/path/legacy.msp","allow_losses":["msp-metadata-ignored"]}}
{"name":"export_image","arguments":{"path":"/absolute/path/fresh.msp"}}
```

The import result includes conversion details: source SHA-256, acknowledged
metadata loss, and explicit unavailable preserving MSP Save. The current project
becomes an unsaved converted copy; Save Project writes PCND. Failed or refused
imports preserve the previous document and its retained source guards. The
MCP session protects the original acquired MSP identity after rename, through
hard-link aliases and after saving the converted project to PCND.

MSP export writes a fresh MSP1 picture only when every rendered pixel is already
opaque black or white. Color, gray and alpha are rejected; no implicit threshold,
dither or white matte is applied. Existing layer-flattening consent still applies.
MSP2 writing, ambiguous zero-length MSP2 rows and original Windows Paint
compatibility remain unqualified. See the [exact profile and oracle evidence](../crates/services/paintcraft-formats/tests/msp-fixtures/README.md).

Equivalent CLI commands are:

```sh
paintcraft-cli import legacy.msp --allow-loss msp-metadata-ignored -o converted.pcnd
paintcraft-cli render converted.pcnd -o fresh.msp
paintcraft-cli paste-image drawing.pcnd legacy.msp \
    --allow-loss msp-metadata-ignored -o placed.pcnd
```

Explicit CLI legacy import also accepts `--frontend classic-v7`, `classic-v10` or
`classic-v11`. MSP flat export uses the Studio profile. Each command retains its
acquired native source identity through output validation; protection across
later operations belongs to the live desktop/MCP workspace, rather than a global
filesystem association.

## ICNS entry conversion and fresh export

Use `inspect_icns` before importing an icon. It returns the acquired source SHA,
ordered record ranges and entry indices, tags, logical sizes, scales, pixel
sizes and qualification. Inspection does not decode pixels or change the
painting. `expect_source_sha256` optionally guards the inspected bytes.

```json
{"name":"inspect_icns","arguments":{"path":"/absolute/path/source.icns"}}
{"name":"import_image","arguments":{"path":"/absolute/path/source.icns","entry":6,"allow_losses":["icns-metadata-ignored","icon-other-entries-not-imported","icon-density-not-retained-in-flat-document"]}}
```

`entry` is zero-based and mandatory; PaintCraft never chooses the largest entry
or substitutes another encoding at the same size. Qualified PNG and small
RGB/mask profiles are imported as a new unsaved flat picture. Unqualified ARGB,
JPEG2000 or ambiguous mask pairings are refused. Actual metadata, density,
unselected-entry, EXIF and palette losses need acknowledgment. The conversion
receipt includes the selected source tag, record/index, logical size, scale and
physical dimensions. The same opened native source remains protected against
alias/rename overwrites, including after saving the converted PCND project.
The source is never adopted as a preserving-save binding.

Fresh output uses explicit PNG tags: `ic07/ic08/ic09` mean 128/256/512 at 1x;
`ic11/ic12/ic13/ic14/ic10` mean 16/32/128/256/512 at 2x. These distinctions remain
explicit even when two types have equal physical dimensions. Legacy small 1x
RGB/mask, small 1x PNG and ARGB output are refused. The reader's small RGB/mask
qualification is the documented PaintCraft/Pillow interoperability profile,
with retained native readback differences; it is not general Apple application
compatibility.

```json
{"name":"export_icns","arguments":{"path":"/absolute/path/copy.icns","types":["ic11","ic12","ic07"],"resampling":"nearest","allow_losses":["icon-resized"]}}
```

Every chosen type is fitted independently from the original canvas onto
transparent padding; `lanczos3` is the default and `nearest` is available. Encoding
uses the exact immutable prepared preview buffers. Actual `flatten`,
`hidden-rgb` and `icon-resized` losses require consent. Eight distinct qualified
types are available; source and aggregate target each admit at most 4,194,304
pixels. Export preserves document pixels, layers, history, dirty state and save
binding. Source aliases, invalid types, missing consent and write failures do
not publish a document change.

Equivalent Studio-profile CLI commands are:

```sh
paintcraft-cli inspect-icns source.icns --json
paintcraft-cli import source.icns --entry 6 -o converted.pcnd \
    --allow-loss icns-metadata-ignored,icon-other-entries-not-imported,icon-density-not-retained-in-flat-document
paintcraft-cli export-icns painting.pcnd -o copy.icns --type ic11 --type ic12 \
    --resampling nearest --allow-loss icon-resized
```

ICNS normal image Open, preserving Save, generic flat export and Classic routes
remain unavailable. See [the format ledger](FORMAT_INTEROPERABILITY.md#icns-internal-backend-increment-2026-10-25)
for the bounded profiles and independent positive/negative evidence.
