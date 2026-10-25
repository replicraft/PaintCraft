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
refuse SVG before reading its content. Desktop/browser SVG controls remain
pending; these options do not enable ordinary SVG Open or SVG export.

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
