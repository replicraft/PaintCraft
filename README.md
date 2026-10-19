# PaintCraft

A faithful Paint.NET / MS Paint reimplementation in Rust.

**Status: usable preview of the editor (roadmap milestone M2, minus reference parity).**
Version **0.1.0** has a CPU editing engine, an exact-integer GPU compositor, a native
`.pcnd` document format, PNG/BMP/JPEG import and
export, a headless CLI and a desktop editor with pencil, brush, eraser, fill, shapes,
rectangle selection, bitmap text, layers, undo/redo, safe save, crash recovery and a
remembered Classic look. Open, Save As and Export use native file dialogs. Nothing here is a released or reference-qualified editor: no
Paint v7/v10/v11 or Paint.NET behavior or UI fidelity has been verified.

## Build and test

```sh
cargo test --workspace          # unit, conformance and end-to-end CLI tests
cargo clippy --workspace --all-targets
cargo run -p paintcraft-cli -- --help
cargo run --release -- [file.pcnd]   # the desktop editor
```

The toolchain is pinned in `rust-toolchain.toml`. The root manifest owns package metadata
and dependency versions; member manifests inherit them through `workspace = true`.
Running `cargo run` at the repository root selects the desktop application.

The desktop attempts GPU document composition on its existing wgpu device and falls back
to the CPU compositor when unavailable or beyond device limits. This initial GPU backend
reads pixels back on a bounded background worker for the shared display boundary;
zero-copy presentation remains pending. Headless CLI builds do not enable GPU or native-dialog dependencies.

To require a real GPU when checking compositing parity:

```sh
PAINTCRAFT_REQUIRE_GPU=1 cargo test -p paintcraft-render --features gpu --test gpu -- --nocapture
```

## Layout

| Path | Purpose |
|---|---|
| `crates/paintcraft-core` | Common IDs, checked geometry, RGBA8 arithmetic, errors, policies and strict JSON under `paintcraft_core::types` |
| `crates/paintcraft-raster` | Sparse copy-on-write tiles, selection coverage, exact shapes, flood fill and bitmap glyph kernels |
| `crates/paintcraft-document` | Persistent document/layer state, metadata and semantic equality |
| `crates/paintcraft-commands` | Typed batch v1/v2 descriptions and structural validation |
| `crates/paintcraft-history` | Recorded snapshots, undo/redo navigation and bounded retention |
| `crates/paintcraft-editor` | Editor sessions, transaction preparation and atomic publication |
| `crates/paintcraft-input` | Normalized pointer events and input descriptors |
| `crates/paintcraft-tools` | Tool state machines, options, previews and editing intentions |
| `crates/paintcraft-render` | Scalar reference compositor and optional wgpu compute compositor |
| `crates/paintcraft-runtime` | Bounded native workers, cancellation and transient job handles |
| `crates/paintcraft-formats` | Canonical PCND reader/writer, PNG/BMP/JPEG codecs, flat export and recovery checkpoints |
| `crates/paintcraft-platform` | Filesystem save adapters and optional native `rfd` dialogs |
| `crates/paintcraft-ui` | Shared view transforms and action outbox |
| `apps/paintcraft` | Desktop controller, settings and egui application (wgpu) |
| `apps/paintcraft-cli` | Headless Clap CLI; each command lives in `src/commands/` |
| `conformance/` | Vendored batch v1 conformance vectors and the canonical specimen |

## Try it

```sh
paintcraft-cli new --width 320 --height 320 -o blank.pcnd
paintcraft-cli inspect blank.pcnd --json          # prints the ticket a batch must carry
paintcraft-cli apply blank.pcnd --operations edits.json -o edited.pcnd \
    --expect-input-sha256 <sha256 from inspect>
paintcraft-cli render edited.pcnd -o preview.png      # or .bmp / .jpg; losses need --allow-loss
paintcraft-cli import photo.jpg -o photo.pcnd
```

Every command that writes a file prints one JSON object (`--json`) with the engine
result and the file-replacement receipt reported separately.

See [docs/IMPLEMENTATION_NOTES.md](docs/IMPLEMENTATION_NOTES.md) for what is implemented,
what is deliberately provisional and what is not started. The licence is in `LICENSE`.
