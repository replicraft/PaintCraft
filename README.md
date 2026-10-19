# PaintCraft

A faithful Paint.NET / MS Paint reimplementation in Rust.

**Status: first editor slice (milestone M1).** There is a CPU-only engine, a native
`.pcnd` document format, a headless CLI and a minimal desktop editor: one canvas, one
pencil, layers, undo/redo and safe save. Nothing here is a released or reference-qualified
editor, and there is no Paint v7/v10/v11 or Paint.NET UI fidelity yet.

## Build and test

```sh
cargo test --workspace          # unit, conformance and end-to-end CLI tests
cargo clippy --workspace --all-targets
cargo run -p paintcraft-cli -- --help
cargo run -p paintcraft --release -- [file.pcnd]   # the desktop editor
```

The toolchain is pinned in `rust-toolchain.toml`.

## Layout

| Path | Purpose |
|---|---|
| `crates/paintcraft-types` | Error codes, checked rectangles, RGBA8 arithmetic, strict JSON |
| `crates/paintcraft-core` | Sparse raster, document, atomic batch execution, session and history, flat render |
| `crates/paintcraft-formats` | Canonical PCND writer and bounded reader, PNG output, the safe-save service |
| `crates/paintcraft-ui` | Window-independent editor controller: view mapping, action outbox, pencil gestures |
| `apps/paintcraft` | Desktop editor (egui + wgpu) |
| `apps/paintcraft-cli` | Headless `paintcraft-cli` binary |
| `conformance/` | Vendored batch v1 conformance vectors and the canonical specimen |

## Try it

```sh
paintcraft-cli new --width 320 --height 320 -o blank.pcnd
paintcraft-cli inspect blank.pcnd --json          # prints the ticket a batch must carry
paintcraft-cli apply blank.pcnd --operations edits.json -o edited.pcnd \
    --expect-input-sha256 <sha256 from inspect>
paintcraft-cli render edited.pcnd -o preview.png
```

Every command that writes a file prints one JSON object (`--json`) with the engine
result and the file-replacement receipt reported separately.

See [docs/IMPLEMENTATION_NOTES.md](docs/IMPLEMENTATION_NOTES.md) for what is implemented,
what is deliberately provisional and what is not started. The licence is in `LICENSE`.
