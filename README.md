# PaintCraft

A faithful Paint.NET / MS Paint reimplementation in Rust.

**Status: engine foundation (milestone M1 slice, minus the GUI).** There is a CPU-only
engine, a native `.pcnd` document format and a headless CLI. There is no desktop
application yet; nothing here is a released or reference-qualified editor.

## Build and test

```sh
cargo test --workspace          # unit, conformance and end-to-end CLI tests
cargo clippy --workspace --all-targets
cargo run -p paintcraft-cli -- --help
```

The toolchain is pinned in `rust-toolchain.toml`.

## Layout

| Path | Purpose |
|---|---|
| `crates/paintcraft-types` | Error codes, checked rectangles, RGBA8 arithmetic, strict JSON |
| `crates/paintcraft-core` | Sparse raster, document, atomic batch execution, session and history, flat render |
| `crates/paintcraft-formats` | Canonical PCND writer and bounded reader, PNG output |
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
