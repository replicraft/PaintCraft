# PCND storage

PaintCraft uses sparse 256 × 256 raster tiles. Missing tiles use the layer's exact
default pixel. A blank canvas therefore needs no pixel chunks. A tile containing
even one changed pixel has a dense RGBA payload: 262,144 bytes for a full tile.
Boundary tiles contain only their valid rows and columns. Hidden RGB under zero
alpha counts as changed data and is preserved.

All normal desktop, browser and CLI project saves now use compact output. Each
block can use exact RGBA runs: a two-byte run length followed by four color bytes.
For example, a full tile with one isolated pixel typically needs three runs (18
bytes), including the unchanged background. The browser uses these runs when they
save space. Native builds additionally try Zstandard level 3 with a 256 KiB maximum
window and choose the smaller encoding. Incompressible blocks remain raw.
Chunks still use ordinary stored ZIP members, grouping blocks toward 4 MiB.

## Measured file sizes

These are actual serialized 1000 × 486 documents produced by
`compact_storage_sizes_and_exact_roundtrip_for_blank_one_pixel_sparse_and_dense`.
The sparse fixture changes one pixel in each of the eight canvas tiles. The dense
fixture contains deterministic xorshift RGBA noise. All figures are bytes.

| Document | Canonical file | Compact file | Raw pixel chunks | Compact pixel chunks |
| --- | ---: | ---: | ---: | ---: |
| Blank | 1,123 | 1,123 | 0 | 0 |
| One changed pixel | 263,591 | 1,467 | 262,144 | 18 |
| Eight sparse changed pixels | 1,946,936 | 3,066 | 1,944,000 | 144 |
| Dense noise | 1,946,931 | 1,946,931 | 1,944,000 | 1,944,000 |

The one-pixel total file is about 180 times smaller; its pixel chunk is about
14,564 times smaller. These examples measure storage, without a save-speed or
runtime-memory improvement claim. Photograph and artwork compression depends on
their content. Compressing persistence does not change dense runtime tile storage.

## Compatibility and safety

The PCND block encodings are `raw`, `zstd`, and the new `rgba-runs`. Earlier builds
cannot read run-encoded blocks. Compatibility with those unpublished builds is
not retained. The reader hashes decoded RGBA bytes, preserving document IDs,
StateIds, layer metadata, defaults, off-canvas layer content and hidden RGB.
`write` and `write_to` remain the original canonical raw writer. Their specimen and
conformance digests remain unchanged. Reading compact output and writing it
canonically reproduces the original raw bytes.

Both native and browser builds expose `write_compact`, `write_compact_to` and a checked streaming
variant. Normal project save paths choose compact output. Native Zstandard encoding uses
the locked library. Compression runs twice so the writer can
prepare descriptors before streaming chunks while retaining only bounded block
scratch and one payload chunk. Compact bytes are reproducible with the pinned
encoder; future compressor upgrades need independent byte-level qualification.

Native and browser builds share the pure Rust `ruzstd` decoder. Before its window
allocation, PaintCraft validates the frame header, dictionary absence, window and
declared content size, every block envelope, exact frame boundary and absence of
trailing or concatenated frames. The decoder must emit exactly the declared block
length; frame checksums, when present, and decoded SHA-256 must match. Frame windows
above 256 KiB are refused. The complete decoded tile set is admitted before any
block inflation. ZIP/member, canvas, block and document limits still apply.

Checked reading observes cancellation between records/blocks and decoded 16 KiB
output chunks; checked writing observes it between bounded block compressions and
output phases. ZIP parsing and an individual codec call are not interruptible.
These checkpoints do not establish native wall-clock cancellation latency.
Failures leave the source document unchanged; callers must discard an unsuccessful
output sink rather than publish it as a saved file.

The browser writes compact pixel runs and reads both run and Zstandard blocks.
Run decoding admits the complete decoded tile set before allocation, checks each
run against the remaining block extent, and rejects short, trailing or malformed
records. A run has at most 65,536 pixels and a block has at most 262,144 decoded
bytes. Hidden RGB is never normalized away.

A 920-point sinusoidal pencil fixture across the 1000 × 486 canvas produces
1,946,936 raw file bytes, a 6,281-byte native compact file, and 9,048 bytes of portable
pixel payload before ZIP/JSON metadata. These are synthetic measurements, not a
measurement of the user's screenshot file.

Tests cover exact roundtrips and canonical digests, mixed raw/compressed boundary
blocks, negative layer origins, hidden RGB, incompressible fallback, decoded quota
denials, every observed checked cancellation, corrupted archive blocks, every
truncated frame, checksum errors, dictionaries, excessive windows, concatenated
frames, trailing bytes and unknown-size frames with short or excessive output.

Raw local logs are retained in ignored `docs/validation/pcnd-storage-2026-10-24/`.

## Validation

The native formats suite passes, including the canonical specimen and conformance
roundtrips. Strict formats Clippy passes. The portable decoder compiles in the
actual browser dependency graph with:

```sh
cargo check -p paintcraft-formats -p paintcraft-web --target wasm32-unknown-unknown
```

Including the browser supplies its existing `uuid/js` randomness feature. An
isolated formats-only Wasm check lacks that preexisting feature and is not claimed
as passing. No live browser import or save-speed benchmark is inferred from this
compile check. Native filesystem/recovery integration is validated separately by
the desktop tests.

## Live browser save check

The production browser build saved an actual 80-segment pencil doodle as a
7,154-byte PCND. Three touched tiles contain 786,432 decoded bytes, represented by
5,274 bytes of run payload (99.3% less pixel payload). Independent Python ZIP,
run decoding and SHA-256 checks recovered 635 black pixels and the white canvas,
with no additional colors. This is a separate test doodle, not the file shown in
the user's download screenshot. Raw evidence is in ignored
`docs/validation/release-polish-2026-10-24/compact-download.json` and
`compact-oracle.json`.
