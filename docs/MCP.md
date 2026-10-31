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

[Editable filters and persistent masks](FILTER_COMMANDS.md) are available through
`apply_operations` and `get_batch_schema`. The seven profiles are Invert,
Brightness/Contrast, Black & White, Threshold, Posterize, Solarize and Box Blur.
They retain parameters, instance identity, source pixels and saved coverage
through Undo/Redo and PCND save/reopen. MCP owns its headless document; it does
not control an open Desktop or browser Studio. Layer masks, styles and the rest
of the pinned 132-entry catalog remain separate work.

`get_batch_schema` returns closed `paintcraft.batch/v5`. The server selects the
lowest grammar required by the actual operation list:

| Operations | Required batch |
|---|---|
| Box Blur Add/Update, `{"kind":"box-blur-v1","radius":2}`; integer radius 0–32 | v5 |
| Black & White/Threshold/Posterize/Solarize Add/Update | v4 |
| `filter.mask.set`, `.from_selection`, `.set_enabled`, `.remove` | v3 |
| Original two-kind Add/Update; scalar enable/reorder/remove/duplicate | v2 |

Parameter fields are required on the wire; draft defaults do not fill omissions.
Earlier closed profiles reject algorithms or tags they do not define. Mask
capture freezes the staged selection once: inactive errors, active empty or
disjoint captures zero. Later selection changes cannot alter it. See the
[atomic capture example](FILTER_COMMANDS.md#mcp-example).

The six [point profiles](architecture/POINT_FILTER_PROFILES.md) preserve alpha,
including transforming hidden RGB. [Box Blur](architecture/SPATIAL_FILTER_INTEGRATION.md)
uses a true finite-source clamped square, premultiplied neighborhood sums and
alpha-changing coverage once after the candidate. Radius zero is exact RGBA
identity but remains a spatial intention. No ICC activation or Photoshop/Palette
byte parity is implied.

`inspect_document` selects distinct closed `paintcraft.mcp-inspect/v4` whenever
any Box is stored, including disabled/radius zero. Otherwise the four newer point
kinds select v3, stored masks select v2, and original unmasked projects retain the
established unversioned shape. CLI uses its separate `paintcraft.inspect/v1…v4`
assessment/document/ticket projections; these schemas are not interchangeable.
Mask summaries remain `{schema,enabled,domain,bands,runs,digest}` with
`a8-bands/v1`, signed source-local endpoint domain, canonical counts and SHA-256;
coverage payloads are never returned.

Results depend on the **final document**, not just the request grammar. Stored
Box requires result/v4; otherwise stored masks require v3, nonempty new filter
aliases require v2, and other results remain exact v1. The corresponding CLI
file-command envelope follows the nested result; MCP returns the engine result
directly. Removing the last Box can downgrade to v1/v2/v3 even in a batch/v5
request. Error-only envelopes remain v1; older closed schemas are unchanged.
For spatial results, actual `document_features` are point, optional mask iff
stored, spatial, then optional source metadata, in that order. A PCND5 header's
mandatory mask **grammar capability** does not imply an actual stored mask.

PCND5 stores Box with algorithm tag 7 plus radius byte; PCND3/4 still refuse tag 7,
even disabled. Point tags 1–6 and older specimen bytes remain unchanged. PCND4
continues to store point-only masks, including disabled/zero fields. Aggregate
mask bands/runs remain 262,144 each; the metadata ceiling remains 8 MiB. Explicit
shared decode and spatial-workspace admission remains required.

Local Box qualification includes actual CLI/stdio workspace and Box groups,
closed schemas over genuine outputs, legacy/future-version regressions and
strict checks. The model, exact Effects kernels, regional renderer with required
Metal, PCND5, Native/Web host and Worker/codec-entry controls pass locally.
[Current qualification counts](architecture/SPATIAL_FILTER_INTEGRATION.md#current-qualification)
keep host, codec and actual browser evidence separate. **Production browser Box
runtime and visible UI qualification are pending.** Earlier six-point production
browser evidence remains scoped to those profiles.

## JPEG background colors

JPEG exports composite transparency over an explicit RGB background. White is
the default. Desktop and web export dialogs expose the background color picker;
CLI uses `--jpeg-matte RRGGBB`, and MCP uses `jpeg_matte`. A leading `#` is also
accepted. The value must contain exactly six RGB hexadecimal digits and is valid
only for JPEG output.

```sh
paintcraft-cli render source.pcnd -o picture.jpg --quality 90 --jpeg-matte 20A0E0 --allow-loss flatten,hidden-rgb,lossy-jpeg,alpha-discarded
```

```json
{"name":"export_image","arguments":{"path":"/absolute/path/picture.jpg","jpeg_matte":"20A0E0","allow_losses":["flatten","hidden-rgb","lossy-jpeg","alpha-discarded"]}}
```

These examples assume an existing project without a retained source attachment.
The background option preserves existing conversion-loss requirements. Failed
format, color or consent checks preserve the destination and painting. MCP checks
request cancellation during rendering, RGB preparation, staged writes and the
existing post-stage publication barrier. Opaque JPEG compression is checked
before and after the call; this does not provide instantaneous preemption or
filesystem compare-and-swap.

Local tests cover all alpha values, unchanged white defaults, seven-language UI
controls, real CLI/MCP exports and cancellation. Actual Chrome numeric RGB edits
and downloads match independently decoded CLI output, with the project bytes,
title and dirty state unchanged. Receipts and excluded calibration attempts are
under `docs/validation/jpeg-matte/`; this does not qualify every browser or JPEG
profile.

### Read-only picture inspection

`inspect_source` works without an open document. It returns
`paintcraft.source-inspection/v1`: original source SHA-256 and byte count,
dimensions, packed storage or component model, palettes, transparency, color
declarations, profile identities, resolution and orientation where available.
The conversion proposal names header-known losses and normalized dimensions.
It is not an import ticket or permission to adopt a document; malformed samples
or compressed data can still fail when decoded. The tool does not assign a
profile, perform color management or retain metadata in a painting.

```json
{"name":"inspect_source","arguments":{"path":"/absolute/path/picture.jpg"}}
```

The corresponding CLI and schema are:

```sh
paintcraft-cli inspect-source picture.jpg --json
paintcraft-cli inspect-source picture.jpg --expect-input-sha256 SHA256 --json
paintcraft-cli schema source-inspection
```

MCP uses `expect_source_sha256` for the optional expected digest. Both interfaces
accept lower input, pixel and adapter-specific metadata allowances; CLI spellings
are `--max-input-bytes`, `--max-pixels` and `--max-metadata-bytes`. These are
logical admission limits, not a total allocation or process-memory guarantee.
In particular, BMP's metadata allowance bounds ICC inspection and excludes its
small fixed palette descriptor. Requests cannot raise the adapter's defaults.

Acquisition uses a captured regular-file handle. The source identity and bytes
are revalidated before returning; replacement, mutation, cancellation and a bad
expected digest fail without changing the document, Undo history or save
binding. This is a bounded observation, not filesystem compare-and-swap or a
persistent guarantee that another process cannot change the file afterward.
Other format families remain unsupported by this tool; their dedicated
inspection workflows remain separate.

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
{"name":"export_image","arguments":{"path":"/absolute/path/fresh-v2.msp","msp_version":"v2"}}
```

The import result includes conversion details: source SHA-256, acknowledged
metadata loss, and explicit unavailable preserving MSP Save. The current project
becomes an unsaved converted copy; Save Project writes PCND. Failed or refused
imports preserve the previous document and its retained source guards. The
MCP session protects the original acquired MSP identity after rename, through
hard-link aliases and after saving the converted project to PCND.

Omitting `msp_version` retains fresh MSP1 output. Explicit `"v1"` selects raw
MSP1 and `"v2"` selects MSP2 row-RLE through the same guarded preparation. Unknown
versions, version options on other formats, and combined DDS/MSP options are
refused. Every rendered pixel must already be opaque black or white. Color,
gray and alpha are rejected; no implicit threshold, dither or white matte is
applied. Actual common `flatten`/`hidden-rgb` losses still need `allow_losses`;
compression itself invents no pixel-loss consent.

Explicit-version responses include `export` metadata (version, pixel/row policy,
dimensions, exact packed/encoded byte counts and output SHA-256), alongside the
normal `save` file-operation receipt. Both resource-admission leases remain live
through staging/publication. Exports do not change history, dirty state or the
saved binding, including an already-dirty opened project. Renamed/hardlinked
sources and stale destinations remain protected. Output limits are 16 MiB,
positive dimensions up to 65,535 per side and 4,194,304 pixels. Fresh RGBA packing
zeros unused tail bits; source header/device metadata is not preserved.

The explicit MSP2 variant is available in native/browser Studio, CLI and MCP.
Studio offers a version chooser and actual encoded-output preview. Native copies
retain the prepared artifact through consent and publication; browser MSP2 uses
its separately bounded 4MP/8-layer isolated Worker, explicit actual-loss consent
and retained download lifetime. MSP1 remains the default with its old bytes and
capacity. Actual desktop/mobile-viewport Chrome proof covers browser preview,
consent, whole-file golden download and unchanged project bytes; it does not
establish physical-device or broader browser qualification. Ambiguous zero-length
MSP2 rows and original Windows Paint compatibility remain unqualified. See the
[base profile](../crates/services/paintcraft-formats/tests/msp-fixtures/README.md),
[MSP2 backend evidence](../crates/services/paintcraft-formats/tests/msp2-writer-fixtures/README.md)
and [actual version consumers](../apps/paintcraft-cli/tests/msp-version-fixtures/README.md).

Equivalent CLI commands are:

```sh
paintcraft-cli import legacy.msp --allow-loss msp-metadata-ignored -o converted.pcnd
paintcraft-cli render converted.pcnd -o fresh.msp
paintcraft-cli render converted.pcnd --msp-version v2 -o fresh-v2.msp
paintcraft-cli paste-image drawing.pcnd legacy.msp \
    --allow-loss msp-metadata-ignored -o placed.pcnd
```

Explicit version selection requires `--render-workers 1` and the default
`--quality`; use `--expect-input-sha256` to require the captured PCND source digest.
MSP1 remains the default native/browser Studio UI export.

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
remain unavailable. See [the format ledger](FORMAT_INTEROPERABILITY.md#icns-selected-entry-conversion-and-fresh-export-2026-10-26)
for the bounded profiles and independent positive/negative evidence.

## Native JPEG XL import

`import_image` accepts the native Studio `native-jxl-srgb8-worker-v1` profile:
metadata-free, static, identity-oriented, original-profile straight sRGB RGBA8,
including hidden RGB and alpha, up to 4,194,304 pixels. ICC, container metadata,
high depth, preview/animation and other unqualified profiles are refused. This
requires the same-build packaged codec helper and matching schema 2 manifest;
helper-free installations fail closed. Generic image Open and Classic JXL reads
remain unavailable. The browser has its own separately qualified explicit JXL
import; native HEIF conversion is described below.

The server captures the current Session owner/state/epoch, keeps decoded work
private, and validates the source identity and cancellation before adoption. RPC
cancellation drains the child supervisor before retiring preparation capacity.
The imported document is unsaved; its intrinsic resident tile leases survive
history and snapshots. Saving or exporting cannot replace the captured source,
including a hardlink alias. This process isolation is not a physical-RSS,
filesystem or descendant-process sandbox.

## Native JPEG XR import

`import_image` accepts `.jxr`, `.wdp` and `.wmp` through the native Studio
`native-jxr-srgb8-worker-v1` profile: single-image unsigned RGB8 without alpha,
ICC, EXIF, orientation or other unqualified metadata. The qualified absent-color-
context profile follows T.832's default sRGB rule. Dimensions are bounded to
4,194,304 pixels; tiled, windowed and other unsupported forms are refused.
The matching same-build helper contract is required; historical imports use
schema 2 and the optional qualified V8 contract uses exact schema 3.

Pass `"allow_losses": ["jxr-resolution-not-retained"]` to acknowledge that the
new document does not retain physical pixel density. Consent is required even
when the source omits density or declares zero: T.832 infers 96 DPI in those
cases. The conversion receipt preserves the original resolution bits separately
from each effective density and its inferred-default flag, plus the color policy,
source SHA, decoder profile and acknowledged loss. Missing or unknown loss
consent leaves the current document unchanged.

Imported pixels become an unsaved document with the same resident ownership,
cancellation and captured-source alias protections as JPEG XL. This is explicit
conversion, not preserving Save or normal image Open. Classic, browser import,
alpha/deep precision remain unavailable. Short headers retain this legacy route.
Qualified long writer headers are selected structurally before launch through
`native-jxr-writer-rgb8-worker-v1`; there is no decoder-error fallback.

`paintcraft-cli --json capabilities --frontend studio` distinguishes `jxr.rgb8.import` (established
short headers), `jxr.writer-rgb8.import` (the explicit long-header subset), and
`jxr.rgb8.lossless.write` (fresh copies). The last two require an exact public V8 or V10
writer profile; historical private helpers are refused. Every native codec row lists
its finite `accepted_helper_profiles`, including compatible public V8 imports.
The historical `required_helper_profile` field is explanatory; the finite array
defines accepted alternatives rather than a version-order rule. Discovery keeps
`installation_preflight_required: true` and
`runtime_installation_verified: false`: it does not verify an installed helper
or claim release qualification. These rows do not enable Classic, browser,
generic Open or preserving Save.

## Native JPEG XR fresh copies

`export_jxr` writes a native Studio fresh copy through an exact qualified
V8/V10 writer installation. Current source, normal helper builds and next
shipping policy select V10 on all six native targets. The
[six-target V10 run](https://github.com/replicraft/PaintCraft/actions/runs/40889561591)
at `5eb0831f` passed mandatory installed App writer controls with 30 scenarios
and float conversion-identity checks. Published 0.1.1 V7/V8 downloads stay
unchanged. Pass `path`, optional
`overwrite` and `allow_losses`. The document must
produce a wholly opaque composite; transparent output is refused instead of
silently applying a matte. For example, a layered opaque document needs its
actual `flatten` loss acknowledged:

```json
{"path":"/absolute/picture.jxr","allow_losses":["flatten"]}
```

The fixed lossless RGB8 writer supports at most 1,048,576 pixels, 65,536 per side
and 64 MiB output. It emits effective 96 DPI through stored zero density fields.
There are no quality, alpha or high-depth options. The result includes the exact
encoder/pixel/density profiles, helper and output SHA256, and the staged save
receipt. This operation preserves the active document, dirty flag and native
save binding; it refuses captured source aliases, stale/cancelled operations and
changed helper contracts before publication. Missing actual loss consent leaves
the destination and session unchanged.

Reading a qualified long writer file uses `import_image` with
`allow_losses:["jxr-resolution-not-retained"]`, producing a new unsaved native
document. Classic/browser conversion, ordinary Open and preserving JPEG XR Save
remain unavailable. The optional helper and actual CLI/MCP path have local Apple
Silicon proof, including the optimized staged source/bundle proof in
local ignored evidence at `docs/validation/jxr-staged-macos-2026-10-26/receipt.json`. The
current source policy selects V10 on all six targets, with both integer JPEG XL
and the fixed JPEG XR writer enabled in normal helper builds. The later six-target
V10 run supplies the installed App writer and float-identity gates; the earlier
V9/V10 matrix and 40 normal macOS ARM debug CLI/MCP tests remain historical
proofs. Portable writer goldens were independently C-qualified, not decoded by
C on every runner. Published 0.1.1 V7/V8 downloads and the tag are unchanged;
this does not claim a new packaged release, original-application compatibility,
signing or notarization.

## Native HEIC/HEIF import

`import_image` accepts `.heic`, `.heif` and `.hif` through the native Studio
`native-heif-srgb8-worker-v1` profile. It supports directly coded explicit sRGB8
color with optional straight8 alpha, up to 4,194,304 pixels and 64 MiB input.
Matching NCLX/VUI declarations are required: identity GBR 4:4:4 or matrix-1
YCbCr 4:4:4/4:2:0, with the qualified ranges and type-0 chroma siting. ICC,
unlabelled color, derived images/grids, transforms, deep/HDR samples and other
unqualified metadata are refused. The matching packaged helper is required.

Pass the losses actually reported by the source:

```json
{
    "path": "/pictures/source.heic",
    "allow_losses": [
        "heif-color-reconstruction",
        "heif-item-names-not-retained"
    ]
}
```

YCbCr reconstruction reports the first loss for range expansion, RGB8
rounding/clipping and, for 4:2:0, bilinear chroma interpolation. Nonempty primary
or alpha item names report the second loss because they are not stored in PCND.
An unnamed full-range identity source has neither loss and accepts an empty
`allow_losses`. Names, source SHA, exact NCLX/range/chroma/alpha declarations and
reconstruction policy remain in the conversion receipt; receipt retention does
not imply document metadata preservation. Missing or unknown consent leaves the
current document unchanged. Set `discard_changes` when intentionally replacing
an existing dirty MCP document, as with other imports.

Imported documents use the existing resident ownership, cancellation and
captured-source alias protections. Saving a converted PCND cannot overwrite its
renamed or hardlinked source. Generic Open, Classic/browser HEIF reads,
preserving Save and HEIF export remain unavailable. See the
[format ledger](FORMAT_INTEROPERABILITY.md#native-heif-srgb8-document-conversion-2026-10-26)
for the independently qualified policy and retained decoder disagreements.

## Native AVIF import

`import_image` accepts `.avif` through the native Studio
`native-avif-srgb8-worker-v1` profile. It requires a complete installation with
the exact AVIF-enabled V5 helper and schema 2 manifest. A V4/helper-free
installation refuses this operation; an integrity error never selects another
helper contract. Normal helper builds now include AVIF. Use an explicitly
`--no-default-features` helper only for V4 compatibility tests.

The initial reader supports directly coded reduced-still identity sRGB8 4:4:4
and optional same-size straight 8-bit monochrome alpha, at most 64 MiB input
and 4,194,304 pixels. Explicit coded/container color declarations must agree:
primary profile 1, full-range primaries 1/transfer 13/matrix 0; alpha profile 0,
full-range CICP 2/2/2 with the straight auxiliary link. Native GBR is reordered
exactly, retaining coded RGB at alpha zero. SDR YCbCr, ICC, HDR/deep precision,
grids, transforms, premultiplication and broader metadata remain refused.

For named primary or alpha items, pass the actual name-loss acknowledgment:

```json
{
    "path": "/pictures/source.avif",
    "allow_losses": ["avif-item-names-not-retained"]
}
```

Names are retained in the conversion assessment receipt, not PCND metadata.
An unnamed identity picture has no conversion loss and accepts an empty
`allow_losses`. Missing or unknown consent, cancellation and stale source/owner
results do not replace the current document. As with other imports, use
`discard_changes` to intentionally replace an existing dirty MCP document.

The shared isolated native job and resident adoption service retain the exact
captured source identity. After PCND Save, renamed/hardlinked source aliases
remain protected against project Save As and image export. Receipts include
exact source SHA, item/sequence/color/alpha declarations, acknowledged losses
and `native-avif-srgb8-worker-v1`; the new document is an unsaved project copy,
not a preserving AVIF binding. Normal Open, Classic/browser AVIF reads and
foreign preserving Save remain denied. Lossy AVIF export is a separate workflow.

The [AVIF qualification record](AVIF_QUALIFICATION.md) distinguishes bit-exact
independent libaom reference pixels, actual local Studio/CLI/MCP and private-prefix
installation checks from unexecuted six-target release and broader-profile work.

## HDR/EXR conversion

Use `inspect_float` to review a supported `.hdr` or `.exr` source without changing
the current painting. A complete native installation with a matching qualified
float-capable helper is required: current source policy selects V10, while finite
V7/V8/V9 compatibility contracts remain supported. Inputs are limited to 16 MiB and 1,048,576 pixels; this converts to
8-bit sRGB rather than retaining an HDR editing document.

Every request supplies explicit settings, for example:

```json
{
    "path": "/pictures/source.hdr",
    "settings": {
        "curve": "reinhard-per-channel",
        "exposure_milli_ev": 0,
        "negative": "reject",
        "alpha_output": "preserve"
    }
}
```

Choose `clip` or `reinhard-per-channel`, exposure from -16000 through 16000
milli-EV, and `reject` or `clip-to-black` for negative samples. To flatten alpha,
use `"alpha_output": "flatten"` with an explicit `"srgb_background": [R, G, B]`;
the backdrop is refused for preserve. Optional `source_assignment` is
`linear-rec709-d65` or `linear-radiance-nominal-ee`; optional `alpha_assignment`
is `premultiplied` or `straight`. Assignments must explicitly match the intended source interpretation.
Unsupported or ambiguous sources are refused unless an admitted explicit
assignment resolves them.

Inspection returns the raw source SHA256, assessment SHA256, conversion
statistics and actual losses. Pass both hashes as `expect_input_sha256` and
`expect_assessment_sha256` to `import_float`, repeat the exact settings, and
acknowledge every reported loss in `allow_losses`. Reinspect after changing the
source or settings. Set `discard_changes` only when intentionally replacing a
dirty painting. Missing consent, stale hashes, cancellation or source changes
leave the existing painting and saved binding unchanged.

CLI equivalents are `inspect-float` and `import-float`, with explicit
`--float-*` settings; import writes a fresh PCND copy. Native Studio offers the
same conversion with a preview. Classic, browser import, ordinary Open and
preserving HDR/EXR Save remain unavailable.

## Native DDS texture export

`export_image` accepts optional nested `dds` settings for explicit native Studio
texture copies. Omit `dds` to keep the existing single-level RGBA8 export;
`"dds": {}` explicitly selects the advanced route with the same pixel defaults.
DDS settings on other output extensions and unknown fields are refused.

| Field | Accepted values and defaults |
| --- | --- |
| `encoding` | `rgba8` (default), `bc1`, `bc2`, `bc3`; BC1 requires every composite alpha to be opaque; BC2/3 use straight alpha |
| `transfer` | `srgb` (default) or `linear8`, which performs real sRGB-to-linear8 conversion |
| `mips` | `none` (default) or `full`; conflicts with `levels` |
| `levels` | Explicit total count 1..17 including the base, further bounded by image geometry; conflicts with `mips` |
| `filter` | `nearest` (default) or `box-stored`; Box requires generated lower levels |
| `effort` | `normal` (default) or `high` for BC; RGBA8 rejects High |

All generated levels derive from the base. Nearest selects base sample centers;
Box averages area/alpha-weighted stored samples, not linear-light sRGB values.
Selecting linear8 changes samples using the pinned deterministic transfer and
quantization policy. It does not merely change a tag. BC compression is lossy;
there is no universal maximum/RMS error guarantee. Actual loss codes are
`dds-block-compression`, `dds-mip-hidden-rgb-normalized` and
`dds-srgb-linear8-quantization`, plus common `flatten`/`hidden-rgb` when reported.
Generating new mip levels alone is not a discarded-source-data loss.

```json
{"name":"export_image","arguments":{"path":"/absolute/path/texture.dds","dds":{"encoding":"bc3","transfer":"srgb","mips":"full","filter":"box-stored","effort":"high"},"allow_losses":["dds-block-compression"]}}
```

Include every additional actual loss reported for the current document; this
example's one consent code suffices only when no other loss is present. The
response retains the file-operation receipt under `save` and includes `export`
metadata for encoding/alpha, transfer policy/quantized sample count, requested
and generated levels, filter/effort, compression policy and encoded SHA-256.
Limits are 4,194,304 base pixels, sides at most 65,536, 17 geometry-bounded
levels and 64 MiB output. Unsupported alpha/texture forms, insufficient budget,
missing consent, cancellation, source aliases or changed destination fingerprints
refuse publication. The session, history, dirty flag and saved binding are
unchanged on success as well as failure. This is a fresh flattened copy, not
preserving DDS Save or an array/cubemap/volume writer.

CLI equivalent:

```sh
paintcraft-cli render painting.pcnd --output texture.dds \
  --dds-encoding bc3 --dds-transfer srgb --dds-mips full \
  --dds-filter box-stored --dds-effort high --allow-loss dds-block-compression
```

Use `--dds-levels N` instead of `--dds-mips` for a bounded explicit count.
The explicit DDS route requires `--render-workers 1` and the default generic
`--quality`; `--dds-effort` controls BC effort. `--expect-input-sha256` binds the
captured input project. Native Studio exposes these choices with a decoded base
preview and loss review. Browser Studio also offers bounded Worker-backed DDS
texture export with separate [source/resource limits](FORMAT_INTEROPERABILITY.md#bounded-web-studio-dds-texture-export-2026-10-27). The
public DDS importer still requires sRGB, so linear8 exports are not silently
reopened as display-sRGB pictures. See the
[DDS profile and evidence](FORMAT_INTEROPERABILITY.md#native-dds-texture-copies-2026-10-26)
for qualification limits.

## DCX export

`export_dcx` writes one document composite as a fresh `.dcx` copy. Choose an
explicit `pcx_mode`: `rgb24`, `indexed-exact` or `quantized256`. Actual flatten,
hidden RGB, alpha-discarded and palette-quantized losses require `allow_losses`.
It preserves the session and dirty state, guards retained native/foreign source
aliases, and uses staged atomic output. It does not map layers to pages or enable
preserving Save. CLI equivalent:

```sh
paintcraft-cli export-dcx painting.pcnd --pcx-mode indexed-exact --output copy.dcx
```

### OpenRaster group conversion

Studio `import_image` accepts flat OpenRaster layers and a bounded neutral-group
subset as a new unsaved PCND project. Grouped files must explicitly use Normal
blending, full opacity and `isolation="auto"`. The
`ora-group-structure-not-retained` acknowledgement accepts losing group hierarchy,
group names and separate group visibility controls; current ancestor visibility
is applied to the imported layers. This is a converted copy, not editable groups
or preserving ORA Save. Unsupported semantics are refused even when hidden.

Regular source files are captured once and revalidated before publication.
The original source identity remains protected after PCND Save and rename,
including hardlink aliases. Canonical `.ora` and extensionless recognized archives
use the same policy. Legacy bounded stream snapshots retain the old flat-only
profile; grouped streams are refused. Missing consent, dirty-project refusal,
source changes and invalid profiles leave the current project untouched.
