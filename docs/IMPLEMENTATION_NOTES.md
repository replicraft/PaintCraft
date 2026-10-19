# Implementation notes

What has been adopted from the PaintCraft specification pack (kept outside version
control under `docs/temp/`), where this implementation deliberately differs, and what
is still provisional. Verified means "covered by tests in this repository"; it never
means native-GUI, GPU, platform or original-application evidence.

## Implemented

| Task | What | Evidence |
|---|---|---|
| T01 | Cargo workspace, pinned toolchain, CPU-only core (no egui/wgpu) | `cargo test --workspace` |
| T04 | Sparse RGBA8 layers: 256×256 copy-on-write tiles, Euclidean addressing, exact default pixel, hidden-RGB preservation, explicit allocation admission | `raster.rs` tests |
| T05 | Immutable document, deterministic UUIDv5 identifiers, StateId / PublicationEpoch / history / dirty-state separation | `document.rs`, `session.rs` tests |
| T06 | Batch v1 parsing, staged atomic execution, pencil and staging budgets, error precedence, semantic no-ops | unit tests and 21 conformance vectors |
| T07 | Linear recorded undo/redo; no algorithm reruns | `session.rs` tests |
| T08 | Canonical PCND v1 writer/reader, flat render, PNG output, `paintcraft-cli` | byte-identical specimen, vector digests, CLI end-to-end tests |
| T29 (minimal) | Bounded PCND probe; unknown version, schema, feature, encoding or group is `capability.unsupported`, never editable | `pcnd.rs` tests, `inspect` |
| T33 (minimal) | `capabilities` lists implemented rows, all `release_qualified: false` | CLI tests |
| T34 | `--expect-input-sha256` checked on the acquired bytes before parsing; engine result and file receipt reported separately | CLI tests |

The CLI's real output validates against the specification pack's JSON schemas and
semantic validators for file-command, file-operation receipts, open assessments,
capabilities, result and error.

## Deliberate differences and choices

- **Own JSON and ZIP code.** `serde_json` accepts duplicate keys and cannot give the
  required UTF-16 canonical key order. A small strict parser and a stored-only ZIP
  reader/writer are used so every accepted input and every output byte is controlled.
  The ZIP reader rejects ZIP64 (so at most 65,534 members, below the pack's 100,000
  entry budget), compression, encryption and data descriptors.
- **Integers only.** JSON numbers with a fraction or exponent are `input.schema`, even
  when integral (`1.0`). The Python oracle's JSON Schema validator accepts those.
- **UUID matching.** Layer ids in a batch are compared as UUID values, so an
  upper-case id matches; the Python oracle compared strings. Neither case is in the vectors.
- **Staging budget.** One batch may stage 268,435,456 pixels (the pack's untrusted-decode
  soft limit; about 1 GiB of tiles). Rendering materializes at most 67,108,864 pixels.
  Both are provisional admission limits, not measured or product limits.
- **`inspect` output.** `paintcraft.inspect/v1` (assessment, document summary, ticket,
  input hash) is this implementation's shape. The pack names the contents but no schema.
- **Identifiers.** Only the deterministic allocator exists. `--deterministic-ids` is
  accepted and has no effect, and `--seed` feeds the document identity. Identical
  `new` parameters therefore produce identical DocumentIds.
- **Loss codes.** PNG export of a multi-layer document needs `--allow-loss flatten`.
  Loss of hidden RGB under alpha zero is not detected yet.
- **Windows.** Durability confirmation syncs the destination directory, which Windows
  does not allow by opening a directory. On Windows a save would report replaced but
  not confirmed (exit 6) until a platform adapter exists. CI covers Linux and macOS only.

## Provisional baselines (no benchmark has been run)

Tile edge 256, a `HashMap` tile index (BF16, BF17), whole-snapshot history sharing
tiles by `Arc` with no eviction or compaction (BF18, T11), a scalar per-pixel
source-over (BF02 not started). Do not describe any of these as measured or optimal.

## Not started

egui/wgpu application and input spike (T02, T09), reference capture (T03), ordered
save journal and recovery (T10), bounded history (T11), groups, text, selections beyond
rectangles, other formats, Wasm effects, Zstd blocks, `paintcraft-web`.
