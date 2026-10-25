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

For ICO/CUR import, provide a zero-based `entry` in `import_image`; no entry is
selected automatically. For DCX, provide a zero-based `page` instead. The two
fields cannot be combined. Import reports losses that must be acknowledged in
`allow_losses`: unselected icon entries use `icon-other-entries-not-imported`,
palette expansion uses `icon-palette-expanded`, and converting a cursor into a
picture uses `cursor-hotspot-not-retained-in-flat-document`. The original source
remains unchanged. CLI import uses the corresponding `--entry` or `--page` flags.

PCX exports accept the optional `pcx_encoding` field: `rgb24` (default),
`indexed-exact` (refuse more than 256 colors), or `indexed-256` (explicitly allow
palette preparation). If colors actually change, include `palette-quantized` in
`allow_losses`; transparency separately requires `alpha-discarded`. Palette
options on another file format are rejected. Exact small palettes stay exact
even when `indexed-256` is requested. Export never changes the open document.

Coordinates are canvas pixels. Colors use `#RRGGBBAA`. Stroke and shape tools default to the top layer; `inspect_document` provides IDs for choosing another layer. Batch operations use the existing command schema, while the server supplies the current concurrency ticket.

Save explicitly before disconnecting: the session is held in memory and ends with the server process. Replacing unsaved work requires `discard_changes: true`. Existing output files require `overwrite: true`, except saving back to the opened project, which checks its captured SHA-256 digest and rejects external changes. Image imports and exports use the CLI Studio format-admission profile. Importing a picture never binds project saves to that source image. Image exports require explicit `allow_losses` acknowledgements when flattening or another conversion would discard information. Exporting does not mark a project saved.

A failed batch publishes no changes. Losing a connection after sending a mutation leaves its outcome uncertain; do not automatically retry it. This initial server does not provide durable request deduplication or recovery across process restarts.
