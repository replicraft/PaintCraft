# PaintCraft

A faithful Paint.NET / MS Paint reimplementation in Rust.

**Status: usable preview of the editor (roadmap milestone M2, minus reference parity).**

Classic UI and themes are the current development focus. Use **Interface** beside
Settings to switch between Classic and Studio; each has its own document workspace.
Choose Save, Discard or Cancel for changed documents when switching. The choice is remembered. See [Studio progress and next steps](docs/STUDIO.md).
Version **0.1.0** has a CPU editing engine, an exact-integer GPU compositor, a native
`.pcnd` document format, PNG/BMP/JPEG import and
export, a headless CLI and a desktop editor with pencil, brush, eraser, fill, shapes,
rectangle selection, antialiased font text, layers, undo/redo, safe save, crash recovery and a
remembered Classic look. Open, Save As and Export use native file dialogs. Nothing here is a released or reference-qualified editor: no
Paint v7/v10/v11 or Paint.NET behavior or UI fidelity has been verified.

## Build and test

```sh
python3 scripts/check_file_lengths.py # repository text files: at most 800 lines
cargo test --workspace          # unit, conformance and end-to-end CLI tests
cargo clippy --workspace --all-targets
cargo run -p paintcraft-cli -- --help
cargo run --release -- [file.pcnd]   # the desktop editor
```

The toolchain is pinned in `rust-toolchain.toml`. The root manifest owns package metadata
and dependency versions; member manifests inherit them through `workspace = true`.
Running `cargo run` at the repository root selects the desktop application.
Handwritten source, tests, schemas, fixtures, and documentation are limited to 800
lines per file; CI checks this limit. Cargo’s generated `Cargo.lock` is exempt.

The desktop attempts GPU document composition on its existing wgpu device and falls back
to the CPU compositor when unavailable or beyond device limits. This initial GPU backend
reads pixels back on a bounded background worker for the shared display boundary;
zero-copy presentation remains pending. Headless CLI builds do not enable GPU or native-dialog dependencies.

To require a real GPU when checking compositing parity:

```sh
PAINTCRAFT_REQUIRE_GPU=1 cargo test -p paintcraft-render --features gpu --test gpu -- --nocapture
```

## Layout

Crates are grouped by responsibility: `foundation` holds shared contracts and
storage, `editing` holds the editing engine, `services` holds rendering and OS/I/O
integration, and `presentation` holds the UI and frontends. These folders organize
the workspace without adding facade crates or changing dependency direction.
All package dependencies remain declared at the workspace root and inherited by
members with `workspace = true`. Application entry points stay in `apps/`.

| Path | Purpose |
|---|---|
| `crates/foundation/paintcraft-core` | Common IDs, checked geometry, RGBA8 arithmetic, errors, policies and strict JSON under `paintcraft_core::types` |
| `crates/foundation/paintcraft-raster` | Sparse copy-on-write tiles, selection coverage, exact shapes, flood fill and bitmap glyph kernels |
| `crates/foundation/paintcraft-document` | Persistent document/layer state, metadata and semantic equality |
| `crates/foundation/paintcraft-input` | Normalized pointer events and input descriptors |
| `crates/editing/paintcraft-commands` | Typed batch v1/v2 descriptions and structural validation |
| `crates/editing/paintcraft-history` | Recorded snapshots, undo/redo navigation and bounded retention |
| `crates/editing/paintcraft-editor` | Editor sessions, transaction preparation and atomic publication |
| `crates/editing/paintcraft-tools` | Tool state machines, options, previews and editing intentions |
| `crates/services/paintcraft-render` | Exact regional/cached CPU compositor, bounded CPU partitions and optional wgpu compute compositor |
| `crates/services/paintcraft-runtime` | Bounded native workers, cancellation and transient job handles |
| `crates/services/paintcraft-formats` | Canonical PCND reader/writer, PNG/BMP/JPEG codecs, flat export and recovery checkpoints |
| `crates/services/paintcraft-platform` | Filesystem save adapters and optional native `rfd` dialogs |
| `crates/presentation/paintcraft-ui` | Shared vector widgets, view transforms and action outbox |
| `crates/presentation/paintcraft-classic` | Classic menus, grouped tool ribbon, palette and skins |
| `crates/presentation/paintcraft-studio` | Studio document tabs, toolbar, tool options and floating Tools/Colors/Layers/History windows |
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
paintcraft-cli render edited.pcnd -o parallel.png --render-workers 4 # explicit CPU worker budget
paintcraft-cli import photo.jpg -o photo.pcnd
```

Every command that writes a file prints one JSON object (`--json`) with the engine
result and the file-replacement receipt reported separately.

See [docs/IMPLEMENTATION_NOTES.md](docs/IMPLEMENTATION_NOTES.md) for what is implemented,
what is deliberately provisional and what is not started. The licence is in `LICENSE`.

CPU exports default to one worker. `--render-workers` accepts 1..64 and uses one
bounded pool; small/cheap images can be faster serially. See the
[CPU partition contract and measurements](docs/PARALLEL_COMPOSITOR.md).
