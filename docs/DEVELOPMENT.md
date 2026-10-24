# Developing PaintCraft

Run the desktop editor from the repository root:

```sh
cargo run --release -- [file.pcnd]
```

Run checks and the headless CLI:

```sh
python3 scripts/check_file_lengths.py
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p paintcraft-cli -- --help
```

The toolchain is pinned in `rust-toolchain.toml`. Package metadata and dependency
versions live in the root workspace manifest. Source files are limited to 800
lines; Markdown and generated data are exempt.

Build and serve the [browser editor](../apps/paintcraft-web/README.md) with its
build script. It shares the editor, file formats, brushes, shapes and Classic
presentation with desktop, with browser-specific file handling and resource
limits. Studio is currently a desktop frontend.

The CLI creates, inspects, edits, imports and renders documents. Each command has
its own module in `apps/paintcraft-cli/src/commands/`.

```sh
paintcraft-cli new --width 320 --height 320 -o blank.pcnd
paintcraft-cli inspect blank.pcnd --json
paintcraft-cli apply blank.pcnd --operations edits.json -o edited.pcnd
paintcraft-cli render edited.pcnd -o preview.png
paintcraft-cli import photo.jpg -o photo.pcnd
```

Batch operations include shape, brush, selection, layer and image transforms.
Export reports representation losses explicitly. See
[implementation notes](IMPLEMENTATION_NOTES.md), [brush media](BRUSH_MEDIA.md),
[localization](LOCALIZATION.md) and [Studio](STUDIO.md) for current scope and limits.

Rendering, residency, history and performance qualification details belong in the
individual design/measurement documents in this folder. To require a real GPU in
compositing parity checks:

```sh
PAINTCRAFT_REQUIRE_GPU=1 cargo test -p paintcraft-render --features gpu --test gpu -- --nocapture
```
